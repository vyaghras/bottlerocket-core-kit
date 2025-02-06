//! The server module owns the API surface.  It interfaces with the datastore through the
//! server::controller module.

mod controller;
mod ephemeral_storage;
mod error;
mod exec;

pub use error::Error;

use actix_web::{
    body::BoxBody, error::ResponseError, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use datastore::{serialize_scalar, Committed, FilesystemDataStore, Key, KeyType, Value};
use error::Result;
use fs2::FileExt;
use http::StatusCode;
use log::info;
use model::ephemeral_storage::{Bind, Init};
use model::generator::{RawSettingsGenerator, Strength};
use model::{ConfigurationFiles, Model, Report, Services, Settings};
use nix::unistd::{chown, Gid};
use serde::{Deserialize, Serialize};
use snafu::{ensure, OptionExt, ResultExt};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{set_permissions, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync;
use thar_be_updates::status::{UpdateStatus, UPDATE_LOCKFILE};
use tokio::process::Command as AsyncCommand;

const BLOODHOUND_BIN: &str = "/usr/bin/bloodhound";
const BLOODHOUND_K8S_CHECKS: &str = "/usr/libexec/cis-checks/kubernetes";
const BLOODHOUND_FIPS_CHECKS: &str = "/usr/libexec/fips-checks/bottlerocket";

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

// sd_notify helper
fn notify_unix_socket_ready() -> Result<()> {
    if env::var_os("NOTIFY_SOCKET").is_some() {
        ensure!(
            Command::new("systemd-notify")
                .arg("--ready")
                .arg("--no-block")
                .status()
                .context(error::SystemdNotifySnafu)?
                .success(),
            error::SystemdNotifyStatusSnafu
        );
        env::remove_var("NOTIFY_SOCKET");
    } else {
        info!("NOTIFY_SOCKET not set, not calling systemd-notify");
    }
    Ok(())
}

// Router

/// This is the primary interface of the module.  It defines the server and application that actix
/// spawns for requests.  It creates a shared datastore handle that can be used by handler methods
/// to interface with the controller.
pub async fn serve<P1, P2, P3>(
    socket_path: P1,
    datastore_path: P2,
    threads: usize,
    socket_gid: Option<Gid>,
    exec_socket_path: P3,
) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
    P3: Into<PathBuf>,
{
    // SharedData gives us a convenient way to make data available to handler methods when it
    // doesn't come from the request itself.  It's easier than the ownership tricks required to
    // pass parameters to the handler methods.
    let shared_data = web::Data::new(SharedData {
        ds: sync::RwLock::new(FilesystemDataStore::new(datastore_path)),
        exec_socket_path: exec_socket_path.into(),
    });

    let http_server = HttpServer::new(move || {
        App::new()
            // This makes the data store available to API methods merely by having a Data
            // parameter.
            .app_data(shared_data.clone())
            // Retrieve the full API model; not all data is writable, so we only support GET.
            .route("/", web::get().to(get_model))
            .service(
                web::scope("/settings")
                    .route("", web::get().to(get_settings))
                    .route("", web::patch().to(patch_settings))
                    .route("/keypair", web::patch().to(patch_settings_key_pair)),
            )
            .service(
                // Transaction support
                web::scope("/tx")
                    .route("/list", web::get().to(get_transaction_list))
                    .route("", web::get().to(get_transaction))
                    .route("", web::delete().to(delete_transaction))
                    .route("/commit", web::post().to(commit_transaction))
                    .route("/apply", web::post().to(apply_changes))
                    .route(
                        "/commit_and_apply",
                        web::post().to(commit_transaction_and_apply),
                    ),
            )
            .service(
                web::scope("/v2")
                    .route("/tx", web::get().to(get_transaction_v2))
                    .route(
                        "/metadata/setting-generators",
                        web::get().to(get_setting_generators_v2),
                    ),
            )
            .service(web::scope("/os").route("", web::get().to(get_os_info)))
            .service(
                web::scope("/metadata")
                    .route("/affected-services", web::get().to(get_affected_services))
                    .route("/setting-generators", web::get().to(get_setting_generators))
                    .route("/templates", web::get().to(get_templates)),
            )
            .service(web::scope("/services").route("", web::get().to(get_services)))
            .service(
                web::scope("/configuration-files")
                    .route("", web::get().to(get_configuration_files)),
            )
            .service(
                web::scope("/actions")
                    .route("/reboot", web::post().to(reboot))
                    .route("/refresh-updates", web::post().to(refresh_updates))
                    .route("/prepare-update", web::post().to(prepare_update))
                    .route("/activate-update", web::post().to(activate_update))
                    .route("/deactivate-update", web::post().to(deactivate_update))
                    .route(
                        "/ephemeral-storage/init",
                        web::post().to(initialize_ephemeral_storage),
                    )
                    .route(
                        "/ephemeral-storage/bind",
                        web::post().to(bind_ephemeral_storage),
                    )
                    .route(
                        "/ephemeral-storage/list-disks",
                        web::get().to(list_ephemeral_storage_disks),
                    )
                    .route(
                        "/ephemeral-storage/list-dirs",
                        web::get().to(list_ephemeral_storage_dirs),
                    ),
            )
            .service(web::scope("/updates").route("/status", web::get().to(get_update_status)))
            .service(web::resource("/exec").route(web::get().to(exec::ws_exec)))
            .service(
                web::scope("/report")
                    .route("", web::get().to(list_reports))
                    .route("/cis", web::get().to(get_cis_report))
                    .route("/fips", web::get().to(get_fips_report)),
            )
    })
    .workers(threads)
    .bind_uds(socket_path.as_ref())
    .context(error::BindSocketSnafu {
        path: socket_path.as_ref(),
    })?;

    // If the socket needs to be chowned to a group to grant further access, that can be passed
    // as a parameter.
    if let Some(gid) = socket_gid {
        chown(socket_path.as_ref(), None, Some(gid)).context(error::SetGroupSnafu { gid })?;
    }

    let mode = 0o0660;
    let perms = Permissions::from_mode(mode);
    set_permissions(socket_path.as_ref(), perms).context(error::SetPermissionsSnafu { mode })?;

    // Notify system manager the UNIX socket has been initialized, so other service units can proceed
    notify_unix_socket_ready()?;

    http_server.run().await.context(error::ServerStartSnafu)
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

// Handler methods called by the router

/// Returns all data in the API model.  If you pass a 'prefix' query string, only field names
/// starting with that prefix will be included.  For example, a prefix of "settings." only returns
/// settings.  Returns a ModelResponse, which contains a serde_json Value instead of a Model so
/// that we can include only matched fields; this is necessary because the 'os' field contains a
/// BottlerocketRelease whose fields aren't optional.  (Its other users depend on those fields.)
async fn get_model(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ModelResponse> {
    // When we query settings, services, etc., we query differently if the user gave a prefix - it
    // means they only want keys that start with their given prefix.  Prefix queries are more
    // forgiving because it's normal to return empty results if the prefix didn't match anything,
    // whereas without prefix matching, we should always have some data to return.  The logic is
    // fairly different, so we branch early.
    if let Some(prefix) = query.get("prefix") {
        return get_model_prefix(data, prefix).await;
    }

    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;

    // Fetch all the data and build a Model.
    let settings = Some(controller::get_settings(&*datastore, &Committed::Live)?);
    let services = Some(controller::get_services(&*datastore)?);
    let configuration_files = Some(controller::get_configuration_files(&*datastore)?);
    let os = Some(controller::get_os_info()?);

    let model = Model {
        settings,
        services,
        configuration_files,
        os,
    };

    // Turn the Model into a Value so we can match the type used when fetching by prefix.
    let val = serde_json::to_value(model).expect("struct to value can't fail");

    Ok(ModelResponse(val))
}

/// Helper for get_model that handles the case of matching a user-specified prefix.
async fn get_model_prefix(data: web::Data<SharedData>, prefix: &str) -> Result<ModelResponse> {
    if prefix.is_empty() {
        return error::EmptyInputSnafu { input: "prefix" }.fail();
    }

    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;

    // Fetch all the data.
    // Note that we don't add a prefix (for example "settings.") to the given prefix before passing
    // it to _prefix methods, like we do in get_settings, because here we're fetching the whole
    // model, not just settings.
    let settings = controller::get_settings_prefix(&*datastore, prefix, &Committed::Live)?;
    let services = controller::get_services_prefix(&*datastore, prefix)?;
    let configuration_files = controller::get_configuration_files_prefix(&*datastore, prefix)?;

    // Build a Model, but exclude 'os' for now.  BottlerocketRelease's fields aren't Option (for
    // good reason - its other users rely on them) so we can't make a BottlerocketRelease with only
    // some fields based on a prefix match.
    let model = Model {
        settings,
        services,
        configuration_files,
        os: None,
    };

    // Turn the Model into a Value so we can insert an "os" value with filtered fields.
    let mut val = serde_json::to_value(model).expect("struct to value can't fail");

    // If the user gave a prefix unrelated to os, this will return None and so we'll leave the None
    // in the model.  Otherwise it'll give us back a Value that's like a BottlerocketRelease but
    // with only the fields matching the prefix.
    if let Some(os) = controller::get_os_prefix(prefix)? {
        // Structs are Objects in serde_json, which have a map of field -> value inside.  We
        // destructure to get it by value, instead of as_object() which gives references.
        let mut map = match val {
            Value::Object(map) => map,
            _ => panic!("structs are always objects"),
        };
        // Insert the filtered result and turn the map back into a Value.
        map.insert("os".to_string(), os);
        val = map.into();
    }

    Ok(ModelResponse(val))
}

// actix-web doesn't support Query for enums, so we use a HashMap and check for the expected keys
// ourselves.
/// Return the live settings from the data store; if 'keys' or 'prefix' are specified in query
/// parameters, return the subset of matching settings.
async fn get_settings(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<SettingsResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;

    let settings = if let Some(keys_str) = query.get("keys") {
        let keys = comma_separated("keys", keys_str)?;
        controller::get_settings_keys(&*datastore, &keys, &Committed::Live)
    } else if let Some(mut prefix) = query.get("prefix") {
        if prefix.is_empty() {
            return error::EmptyInputSnafu { input: "prefix" }.fail();
        }
        // When retrieving from /settings, the settings prefix is implied, so we add it if it
        // wasn't given.
        let with_prefix = format!("settings.{}", prefix);
        if !prefix.starts_with("settings") {
            prefix = &with_prefix;
        }
        controller::get_settings_prefix(&*datastore, prefix, &Committed::Live)
            .map(|opt| opt.unwrap_or_default())
    } else {
        controller::get_settings(&*datastore, &Committed::Live)
    }?;

    Ok(SettingsResponse(settings))
}

/// Apply the requested settings to the pending data store
async fn patch_settings(
    settings: web::Json<Settings>,
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<HttpResponse> {
    let transaction = transaction_name(&query);
    let strength = query_strength(&query)?;
    let mut datastore = data.ds.write().ok().context(error::DataStoreLockSnafu)?;
    controller::set_settings(&mut *datastore, &settings, transaction, strength)?;
    Ok(HttpResponse::NoContent().finish()) // 204
}

// Apply the requested settings in Key Value pair.
async fn patch_settings_key_pair(
    settings: web::Json<SetKeyPairSettings>,
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<HttpResponse> {
    // Convert to a Map of Key Value pairs.
    let settings_key_pair_map = construct_key_pair_map(&settings.request_payload)?;
    let transaction = transaction_name(&query);
    let strength = query_strength(&query)?;
    let mut datastore = data.ds.write().ok().context(error::DataStoreLockSnafu)?;
    // We massage the values in the input key pair map.
    // The data store deserialization code understands how to turn the key names
    // (a.b.c) and serialized values into the nested Settings structure.
    let settings_model = datastore::deserialization::from_map(&settings_key_pair_map)
        .context(error::DeserializeMapSnafu)?;
    controller::set_settings(&mut *datastore, &settings_model, transaction, strength)?;
    Ok(HttpResponse::NoContent().finish()) // 204
}

async fn get_transaction_list(data: web::Data<SharedData>) -> Result<TransactionListResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
    let data = controller::list_transactions(&*datastore)?;
    Ok(TransactionListResponse(data))
}

/// Get any pending settings in the given transaction, or the "default" transaction if unspecified.
async fn get_transaction(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<SettingsResponse> {
    let transaction = transaction_name(&query);
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
    let data = controller::get_transaction(&*datastore, transaction)?;

    Ok(SettingsResponse(data))
}

/// Get any pending settings in the given transaction, or the "default" transaction if unspecified.
async fn get_transaction_v2(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<SettingsResponseWithMetadata> {
    let transaction = transaction_name(&query);
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
    let settings = controller::get_transaction(&*datastore, transaction)?;
    let transaction_metadata =
        controller::get_transaction_metadata(&*datastore, transaction, None)?;

    let data = SettingsWithMetadata {
        settings,
        metadata: transaction_metadata.inner,
    };

    Ok(SettingsResponseWithMetadata(data))
}

/// Delete the given transaction, or the "default" transaction if unspecified.
async fn delete_transaction(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ChangedKeysResponse> {
    let transaction = transaction_name(&query);
    let mut datastore = data.ds.write().ok().context(error::DataStoreLockSnafu)?;
    let deleted = controller::delete_transaction(&mut *datastore, transaction)?;
    Ok(ChangedKeysResponse(deleted))
}

/// Save settings changes from the given transaction, or the "default" transaction if unspecified,
/// to the live data store.  Returns the list of changed keys.
async fn commit_transaction(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ChangedKeysResponse> {
    let transaction = transaction_name(&query);
    let mut datastore = data.ds.write().ok().context(error::DataStoreLockSnafu)?;

    let changes = controller::commit_transaction::<datastore::filesystem::FilesystemDataStore>(
        &mut *datastore,
        transaction,
    )?;

    if changes.is_empty() {
        return error::CommitWithNoPendingSnafu.fail();
    }

    Ok(ChangedKeysResponse(changes))
}

/// Starts settings appliers for any changes that have been committed to the data store.  This
/// updates config files, runs restart commands, etc.
async fn apply_changes(query: web::Query<HashMap<String, String>>) -> Result<HttpResponse> {
    if let Some(keys_str) = query.get("keys") {
        let keys = comma_separated("keys", keys_str)?;
        controller::apply_changes(Some(&keys))?;
    } else {
        controller::apply_changes(None as Option<&HashSet<&str>>)?;
    }

    Ok(HttpResponse::NoContent().json(()))
}

/// Usually you want to apply settings changes you've committed, so this is a convenience method to
/// perform both a commit and an apply.  Commits the given transaction, or the "default"
/// transaction if unspecified.
async fn commit_transaction_and_apply(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ChangedKeysResponse> {
    let transaction = transaction_name(&query);
    let mut datastore = data.ds.write().ok().context(error::DataStoreLockSnafu)?;

    let changes = controller::commit_transaction::<datastore::filesystem::FilesystemDataStore>(
        &mut *datastore,
        transaction,
    )?;

    if changes.is_empty() {
        return error::CommitWithNoPendingSnafu.fail();
    }

    let key_names = changes.iter().map(|k| k.name()).collect();
    controller::apply_changes(Some(&key_names))?;

    Ok(ChangedKeysResponse(changes))
}

/// Returns information about the OS image, like variant and version.  If you pass a 'prefix' query
/// string, only field names starting with that prefix will be included.  Returns a
/// BottlerocketReleaseResponse, which contains a serde_json Value instead of a BottlerocketRelease
/// so that we can include only matched fields.
async fn get_os_info(
    query: web::Query<HashMap<String, String>>,
) -> Result<BottlerocketReleaseResponse> {
    let os = if let Some(mut prefix) = query.get("prefix") {
        if prefix.is_empty() {
            return error::EmptyInputSnafu { input: "prefix" }.fail();
        }
        // When retrieving from /os, the "os" prefix is implied, so we add it if it wasn't given.
        let with_prefix = format!("os.{}", prefix);
        if !prefix.starts_with("os") {
            prefix = &with_prefix;
        }
        controller::get_os_prefix(prefix)?.unwrap_or_else(|| Value::Object(serde_json::Map::new()))
    } else {
        let os = controller::get_os_info()?;
        serde_json::to_value(os).expect("struct to value can't fail")
    };

    Ok(BottlerocketReleaseResponse(os))
}

/// Get the affected services for a list of data keys
async fn get_affected_services(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<MetadataResponse> {
    if let Some(keys_str) = query.get("keys") {
        let data_keys = comma_separated("keys", keys_str)?;
        let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
        let resp =
            controller::get_metadata_for_data_keys(&*datastore, "affected-services", &data_keys)?;

        Ok(MetadataResponse(resp))
    } else {
        error::MissingInputSnafu { input: "keys" }.fail()
    }
}

/// Get all settings that have setting-generator metadata
async fn get_setting_generators(data: web::Data<SharedData>) -> Result<MetadataResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
    let metadata_for_keys =
        controller::get_settings_generator_metadata(&*datastore, "setting-generator")?;
    let mut resp: HashMap<String, Value> = HashMap::new();

    for (key, value) in metadata_for_keys.iter() {
        let setting_generator: RawSettingsGenerator = serde_json::from_value(value.clone())
            .context(error::DeserializeSettingsGeneratorSnafu)?;

        // Not return weak setting generators because the callers using
        // v1 of this api are not aware of the strength.
        if setting_generator.strength == Strength::Strong {
            resp.insert(
                key.to_string(),
                serde_json::Value::String(setting_generator.command),
            );
        }
    }

    Ok(MetadataResponse(resp))
}

async fn get_setting_generators_v2(data: web::Data<SharedData>) -> Result<MetadataResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
    let resp = controller::get_settings_generator_metadata(&*datastore, "setting-generator")?;
    Ok(MetadataResponse(resp))
}

/// Get the template metadata for a list of data keys
async fn get_templates(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<MetadataResponse> {
    if let Some(keys_str) = query.get("keys") {
        let data_keys = comma_separated("keys", keys_str)?;
        let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;
        let resp = controller::get_metadata_for_data_keys(&*datastore, "template", &data_keys)?;

        Ok(MetadataResponse(resp))
    } else {
        error::MissingInputSnafu { input: "keys" }.fail()
    }
}

/// Get all services, or if 'names' is specified, services with those names.  If you pass a
/// 'prefix' query string, only services starting with that prefix will be included.
async fn get_services(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ServicesResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;

    let resp = if let Some(names_str) = query.get("names") {
        let names = comma_separated("names", names_str)?;
        controller::get_services_names(&*datastore, &names, &Committed::Live)
    } else if let Some(mut prefix) = query.get("prefix") {
        if prefix.is_empty() {
            return error::EmptyInputSnafu { input: "prefix" }.fail();
        }
        // When retrieving from /services, the services prefix is implied, so we add it if it
        // wasn't given.
        let with_prefix = format!("services.{}", prefix);
        if !prefix.starts_with("services") {
            prefix = &with_prefix;
        }
        controller::get_services_prefix(&*datastore, prefix).map(|opt| opt.unwrap_or_default())
    } else {
        controller::get_services(&*datastore)
    }?;

    Ok(ServicesResponse(resp))
}

/// Get all configuration files, or if 'names' is specified, configuration files with those names.
/// If you pass a 'prefix' query string, only configuration files starting with that prefix will be
/// included.
async fn get_configuration_files(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<SharedData>,
) -> Result<ConfigurationFilesResponse> {
    let datastore = data.ds.read().ok().context(error::DataStoreLockSnafu)?;

    let resp = if let Some(names_str) = query.get("names") {
        let names = comma_separated("names", names_str)?;
        controller::get_configuration_files_names(&*datastore, &names, &Committed::Live)
    } else if let Some(mut prefix) = query.get("prefix") {
        if prefix.is_empty() {
            return error::EmptyInputSnafu { input: "prefix" }.fail();
        }
        // When retrieving from /configuration-files, the configuration-files prefix is implied, so
        // we add it if it wasn't given.
        let with_prefix = format!("configuration-files.{}", prefix);
        if !prefix.starts_with("configuration-files") {
            prefix = &with_prefix;
        }
        controller::get_configuration_files_prefix(&*datastore, prefix)
            .map(|opt| opt.unwrap_or_default())
    } else {
        controller::get_configuration_files(&*datastore)
    }?;

    Ok(ConfigurationFilesResponse(resp))
}

/// Get the update status from 'thar-be-updates'
async fn get_update_status() -> Result<UpdateStatusResponse> {
    let lockfile = File::create(UPDATE_LOCKFILE).context(error::UpdateLockOpenSnafu)?;
    lockfile
        .try_lock_shared()
        .context(error::UpdateShareLockSnafu)?;
    let result = thar_be_updates::status::get_update_status(&lockfile);
    match result {
        Ok(update_status) => Ok(UpdateStatusResponse(update_status)),
        Err(e) => match e {
            thar_be_updates::error::Error::NoStatusFile { .. } => {
                error::UninitializedUpdateStatusSnafu.fail()
            }
            _ => error::UpdateSnafu.fail(),
        },
    }
}

/// Refreshes the list of updates and checks if an update is available matching the configured version lock
async fn refresh_updates() -> Result<HttpResponse> {
    controller::dispatch_update_command(&["refresh"])
}

/// Prepares update by downloading the images to the staging partition set
async fn prepare_update() -> Result<HttpResponse> {
    controller::dispatch_update_command(&["prepare"])
}

/// "Activates" an already staged update by bumping the priority bits on the staging partition set
async fn activate_update() -> Result<HttpResponse> {
    controller::dispatch_update_command(&["activate"])
}

/// "Deactivates" an already activated update by rolling back actions done by 'activate-update'
async fn deactivate_update() -> Result<HttpResponse> {
    controller::dispatch_update_command(&["deactivate"])
}

/// Reboots the machine
async fn reboot() -> Result<HttpResponse> {
    debug!("Rebooting now");
    let output = Command::new("/usr/bin/systemctl")
        .arg("reboot")
        .arg("--check-inhibitors=yes")
        .output()
        .context(error::ShutdownSnafu)?;
    ensure!(
        output.status.success(),
        error::RebootSnafu {
            exit_code: match output.status.code() {
                Some(code) => code,
                None => output.status.signal().unwrap_or(1),
            },
            stderr: String::from_utf8_lossy(&output.stderr),
        }
    );
    Ok(HttpResponse::NoContent().finish())
}

/// Gets the set of report types supported by this host.
async fn list_reports() -> Result<ReportListResponse> {
    // Add each report to list response when adding a new handler
    let data = vec![Report {
        name: "cis".to_string(),
        description: "CIS Bottlerocket Benchmark".to_string(),
    }];
    Ok(ReportListResponse(data))
}

/// Gets the Bottlerocket CIS benchmark report.
async fn get_cis_report(query: web::Query<HashMap<String, String>>) -> Result<HttpResponse> {
    let mut cmd = AsyncCommand::new(BLOODHOUND_BIN);

    // Check for requested level, default is 1
    if let Some(level) = query.get("level") {
        cmd.arg("-l").arg(level);
    }

    // Check for requested format, default is text
    if let Some(format) = query.get("format") {
        cmd.arg("-f").arg(format);
    }

    if let Some(report_type) = query.get("type") {
        if report_type == "kubernetes" {
            cmd.arg("-c").arg(BLOODHOUND_K8S_CHECKS);
        }
    }

    let output = cmd.output().await.context(error::ReportExecSnafu)?;
    ensure!(
        output.status.success(),
        error::ReportResultSnafu {
            exit_code: match output.status.code() {
                Some(code) => code,
                None => output.status.signal().unwrap_or(1),
            },
            stderr: String::from_utf8_lossy(&output.stderr),
        }
    );
    Ok(HttpResponse::Ok()
        .content_type("application/text")
        .body(String::from_utf8_lossy(&output.stdout).to_string()))
}

/// Gets the FIPS Security Policy report.
async fn get_fips_report(query: web::Query<HashMap<String, String>>) -> Result<HttpResponse> {
    let mut cmd = AsyncCommand::new(BLOODHOUND_BIN);

    // Check for requested format, default is text
    if let Some(format) = query.get("format") {
        cmd.arg("-f").arg(format);
    }

    cmd.arg("-c").arg(BLOODHOUND_FIPS_CHECKS);

    let output = cmd.output().await.context(error::ReportExecSnafu)?;
    ensure!(
        output.status.success(),
        error::ReportResultSnafu {
            exit_code: match output.status.code() {
                Some(code) => code,
                None => output.status.signal().unwrap_or(1),
            },
            stderr: String::from_utf8_lossy(&output.stderr),
        }
    );
    Ok(HttpResponse::Ok()
        .content_type("application/text")
        .body(String::from_utf8_lossy(&output.stdout).to_string()))
}

/// Configure ephemeral storage (raid & format, or just format for single disk)
async fn initialize_ephemeral_storage(cfg: web::Json<Init>) -> Result<HttpResponse> {
    ephemeral_storage::initialize(cfg.0.filesystem, cfg.0.disks)
        .context(error::EphemeralInitializeSnafu {})?;
    Ok(HttpResponse::NoContent().finish()) // 204
}
/// Bind directories to ephemeral storage (mount array, bind, and unmount)
async fn bind_ephemeral_storage(cfg: web::Json<Bind>) -> Result<HttpResponse> {
    let os_info = controller::get_os_info()?;
    ephemeral_storage::bind(&os_info.variant_id, cfg.0.targets)
        .context(error::EphemeralBindSnafu {})?;
    Ok(HttpResponse::NoContent().finish()) // 204
}

/// Lists the known ephemeral disks that can be configured.
async fn list_ephemeral_storage_disks(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let disks =
        ephemeral_storage::ephemeral_devices().context(error::EphemeralListDisksSnafu {})?;

    let mut text_response = String::new();
    for disk in &disks {
        text_response.push_str(disk);
        text_response.push('\n');
    }
    list_ephemeral_response(req, query, disks, text_response).await
}

/// Lists the known ephemeral disks that can be configured.
async fn list_ephemeral_storage_dirs(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let os_info = controller::get_os_info()?;

    let allowed_dirs = ephemeral_storage::allowed_bind_dirs(&os_info.variant_id);
    let mut text_response = String::new();
    for dir in &allowed_dirs.allowed_exact {
        text_response.push_str(dir);
        text_response.push('\n');
    }

    let allowed: Vec<String> = allowed_dirs
        .allowed_exact
        .iter()
        .map(|x| String::from(*x))
        .collect();
    list_ephemeral_response(req, query, allowed, text_response).await
}

// Responds to a list request with the text or JSON response depending on the query format.
async fn list_ephemeral_response(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    items: Vec<String>,
    text_response: String,
) -> Result<HttpResponse> {
    match query
        .get("format")
        .unwrap_or(&String::from("text"))
        .as_str()
    {
        "json" => Ok(EphemeralListResponse(items).respond_to(&req)),
        "text" => Ok(HttpResponse::Ok()
            .content_type("application/text")
            .body(text_response)),
        _ => Ok(HttpResponse::BadRequest().body("unsupported format")),
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

// Helpers for handler methods called by the router

fn comma_separated<'a>(key_name: &'static str, input: &'a str) -> Result<HashSet<&'a str>> {
    if input.is_empty() {
        return error::EmptyInputSnafu { input: key_name }.fail();
    }
    Ok(input.split(',').collect())
}

fn transaction_name(query: &web::Query<HashMap<String, String>>) -> &str {
    query.get("tx").map(String::as_str).unwrap_or("default")
}

fn query_strength(query: &web::Query<HashMap<String, String>>) -> Result<Strength> {
    if let Some(strength) = query.get("strength") {
        Ok(
            Strength::from_str(strength).context(error::InvalidStrengthSnafu {
                strength: strength.to_string(),
            })?,
        )
    } else {
        Ok(Strength::default())
    }
}

// Helpers methods for the 'set' API
fn construct_key_pair_map(settings_key_pair_vec: &Vec<String>) -> Result<HashMap<Key, String>> {
    let mut settings_key_pair_map = HashMap::new();
    for settings_key_pair in settings_key_pair_vec {
        // If we see an invalid key pair. We will return the result with an error immediately.
        let (raw_key, value) =
            settings_key_pair
                .split_once('=')
                .ok_or_else(|| Error::InvalidKeyPair {
                    input: settings_key_pair.clone(),
                })?;
        let mut key = Key::new(KeyType::Data, raw_key)
            .context(error::InvalidPrefixSnafu { prefix: raw_key })?;
        // Add "settings" prefix if the user didn't give a known prefix, to ease usage
        let key_prefix = &key.segments()[0];

        if key_prefix != "settings" {
            let mut segments = key.segments().clone();
            segments.insert(0, "settings".to_string());
            key = Key::from_segments(KeyType::Data, &segments)
                .context(error::InvalidPrefixSnafu { prefix: raw_key })?;
        }

        settings_key_pair_map.insert(key, value.to_string());
    }
    trace!("Key=Value map: {:#?}", settings_key_pair_map);
    // We massage the map values to determine type of each input
    massage_set_input(settings_key_pair_map)
}

/// We want the key=val form of 'set' to be as simple as possible; we don't want users to have to
/// annotate or structure their input too much just to tell us the data type, but unfortunately
/// knowledge of the data type is required to deserialize with the current datastore ser/de code.
///
/// To simplify usage, we use some heuristics to determine the type of each input.  We try to parse
/// each value as a number and boolean, and if those fail, we assume a string.  (API communication
/// is in JSON form, limiting the set of types; the API doesn't allow arrays or null, and "objects"
/// (maps) are represented natively through our nested tree-like settings structure.)
///
/// If this goes wrong -- for example the user wants a string "42" -- we'll get a deserialization
/// error, and can print a clear error and request the user use JSON input form to handle
/// situations with more complex types.g
///
/// If you have an idea for how to improve deserialization so we don't have to do this, please say!
fn massage_set_input(input_map: HashMap<Key, String>) -> Result<HashMap<Key, String>> {
    // Deserialize the given value into the matching Rust type.  When we find a matching type, we
    // serialize back out to the data store format, which is required to build a Settings object
    // through the data store deserialization code.
    let mut massaged_map = HashMap::with_capacity(input_map.len());
    for (key, in_val) in input_map {
        let serialized = if let Ok(b) = serde_json::from_str::<bool>(&in_val) {
            serialize_scalar(&b).context(error::SerializeSnafu)?
        } else if let Ok(u) = serde_json::from_str::<u64>(&in_val) {
            trace!("Serializing scalar of type u64");
            serialize_scalar(&u).context(error::SerializeSnafu)?
        } else if let Ok(f) = serde_json::from_str::<f64>(&in_val) {
            trace!("Serializing scalar of type f64");
            serialize_scalar(&f).context(error::SerializeSnafu)?
        } else {
            trace!("Serializing scalar of type string");
            // No deserialization, already a string, just serialize
            serialize_scalar(&in_val).context(error::SerializeSnafu)?
        };
        massaged_map.insert(key, serialized);
    }
    trace!("Massaged key=value input: {:#?}", massaged_map);
    Ok(massaged_map)
}

// Can also override `render_response` if we want to change headers, content type, etc.
impl ResponseError for error::Error {
    /// Maps our error types to the HTTP error code they should return.
    fn error_response(&self) -> HttpResponse {
        use error::Error::*;
        let status_code = match self {
            // 400 Bad Request
            MissingInput { .. } => StatusCode::BAD_REQUEST,
            EmptyInput { .. } => StatusCode::BAD_REQUEST,
            NewKey { .. } => StatusCode::BAD_REQUEST,
            ReportTypeMissing { .. } => StatusCode::BAD_REQUEST,
            Serialize { .. } => StatusCode::BAD_REQUEST,
            DeserializeMap { .. } => StatusCode::BAD_REQUEST,
            InvalidPrefix { .. } => StatusCode::BAD_REQUEST,
            DeserializeJson { .. } => StatusCode::BAD_REQUEST,
            InvalidKeyPair { .. } => StatusCode::BAD_REQUEST,

            // 404 Not Found
            MissingData { .. } => StatusCode::NOT_FOUND,
            ListKeys { .. } => StatusCode::NOT_FOUND,
            UpdateDoesNotExist { .. } => StatusCode::NOT_FOUND,
            NoStagedImage { .. } => StatusCode::NOT_FOUND,
            UninitializedUpdateStatus { .. } => StatusCode::NOT_FOUND,

            // 422 Unprocessable Entity
            CommitWithNoPending => StatusCode::UNPROCESSABLE_ENTITY,
            ReportNotSupported { .. } => StatusCode::UNPROCESSABLE_ENTITY,

            // 423 Locked
            UpdateShareLock { .. } => StatusCode::LOCKED,
            UpdateLockHeld { .. } => StatusCode::LOCKED,

            // 409 Conflict
            DisallowCommand { .. } => StatusCode::CONFLICT,

            // 500 Internal Server Error
            DataStoreLock => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseSerialization { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            BindSocket { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ServerStart { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ListedKeyNotPresent { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            DataStore { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Deserialization { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            DataStoreSerialization { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            CommandSerialization { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            EphemeralBind { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            EphemeralInitialize { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            EphemeralListDisks { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            InvalidMetadata { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigApplierFork { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigApplierStart { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigApplierStdin {} => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigApplierWait { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigApplierWrite { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            SystemdNotify { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            SystemdNotifyStatus {} => StatusCode::INTERNAL_SERVER_ERROR,
            SetPermissions { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            SetGroup { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            SettingsToJson { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ReleaseData { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Shutdown { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Reboot { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            UpdateDispatcher { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            UpdateError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            UpdateStatusParse { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            UpdateInfoParse { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            UpdateLockOpen { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ReportExec { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ReportResult { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            DeserializeSettingsGenerator { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            DeserializeStrength { .. } => StatusCode::BAD_REQUEST,
            InvalidStrength { .. } => StatusCode::BAD_REQUEST,
            DisallowStrongToWeakStrength { .. } => StatusCode::BAD_REQUEST,
            ParseStrength { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };

        HttpResponse::build(status_code).body(self.to_string())
    }
}

/// A struct used by actix for the 'set' subcommand when user-supplied values are key-pairs.
#[derive(Serialize, Deserialize)]
struct SetKeyPairSettings {
    request_payload: Vec<String>,
}

/// SharedData is responsible for any data needed by web handlers that isn't provided by the client
/// in the request.
pub(crate) struct SharedData {
    ds: sync::RwLock<FilesystemDataStore>,
    exec_socket_path: PathBuf,
}

/// Helper macro for implementing the actix-web Responder trait for a type.
/// $for: the type for which we implement Responder.
/// $self: just pass "self"  (macro hygiene requires this)
/// $serialize_expr: the thing to serialize for a response; this is just "self" again if $for
///    implements Serialize, or is "self.0" for a newtype over something implementing Serialize
macro_rules! impl_responder_for {
    ($for:ident, $self:ident, $serialize_expr:expr) => (
        impl Responder for $for {
            type Body = BoxBody;
            fn respond_to($self, _req: &HttpRequest) -> HttpResponse {
                let body = match serde_json::to_string(&$serialize_expr) {
                    Ok(s) => s,
                    Err(e) => return Error::ResponseSerialization { source: e }.into(),
                };
                HttpResponse::Ok()
                    .content_type("application/json")
                    .body(body)
            }
        }
    )
}

/// This lets us respond from our handler methods with a model (or Result<model>), where "model" is
/// a serde_json::Value corresponding to the Model struct.
///
/// This contains a serde_json::Value instead of a Model to support prefix queries; if the user
/// gives a prefix that doesn't match all BottlerocketRelease fields, we can't construct a
/// BottlerocketRelease since its fields aren't Option; using a Value lets us return the same
/// structure, just not including fields the user doesn't want to see.  (Trying to deserialize
/// those results into a Model/BottlerocketRelease would fail, so it's just intended for viewing.)
struct ModelResponse(serde_json::Value);
impl_responder_for!(ModelResponse, self, self.0);

/// This lets us respond from our handler methods with the Settings and Metadata
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct SettingsWithMetadata {
    settings: Settings,
    metadata: HashMap<String, HashMap<String, String>>,
}

struct SettingsResponseWithMetadata(SettingsWithMetadata);
impl_responder_for!(SettingsResponseWithMetadata, self, self.0);

/// This lets us respond from our handler methods with a Settings (or Result<Settings>)
struct SettingsResponse(Settings);
impl_responder_for!(SettingsResponse, self, self.0);

/// This lets us respond from our handler methods with a release (or Result<release>), where
/// "release" is a serde_json::Value corresponding to the BottlerocketRelease struct.
///
/// This contains a serde_json::Value instead of a BottlerocketRelease to support prefix queries;
/// if the user gives a prefix that doesn't match all BottlerocketRelease fields, we can't
/// construct a BottlerocketRelease since its fields aren't Option; using a Value lets us return
/// the same structure, just not including fields the user doesn't want to see.  (Trying to
/// deserialize those results into a BottlerocketRelease would fail, so it's just intended for
/// viewing.)
struct BottlerocketReleaseResponse(serde_json::Value);
impl_responder_for!(BottlerocketReleaseResponse, self, self.0);

/// This lets us respond from our handler methods with a HashMap (or Result<HashMap>) for metadata
struct MetadataResponse(HashMap<String, Value>);
impl_responder_for!(MetadataResponse, self, self.0);

/// This lets us respond from our handler methods with a Services (or Result<Services>)
struct ServicesResponse(Services);
impl_responder_for!(ServicesResponse, self, self.0);

/// This lets us respond from our handler methods with a UpdateStatus (or Result<UpdateStatus>)
struct UpdateStatusResponse(UpdateStatus);
impl_responder_for!(UpdateStatusResponse, self, self.0);

/// This lets us respond from our handler methods with a ConfigurationFiles (or
/// Result<ConfigurationFiles>)
struct ConfigurationFilesResponse(ConfigurationFiles);
impl_responder_for!(ConfigurationFilesResponse, self, self.0);

struct ChangedKeysResponse(HashSet<Key>);
impl_responder_for!(ChangedKeysResponse, self, self.0);

struct TransactionListResponse(HashSet<String>);
impl_responder_for!(TransactionListResponse, self, self.0);

struct ReportListResponse(Vec<Report>);
impl_responder_for!(ReportListResponse, self, self.0);

struct EphemeralListResponse(Vec<String>);
impl_responder_for!(EphemeralListResponse, self, self.0);
