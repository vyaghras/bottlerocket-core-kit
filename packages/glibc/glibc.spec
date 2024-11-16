Name: %{_cross_os}glibc
Version: 2.40
Release: 1%{?dist}
Epoch: 1
Summary: The GNU libc libraries
License: LGPL-2.1-or-later AND (LGPL-2.1-or-later WITH GCC-exception-2.0) AND GPL-2.0-or-later AND (GPL-2.0-or-later WITH GCC-exception-2.0) AND BSD-3-Clause AND ISC
URL: http://www.gnu.org/software/glibc/
Source0: https://ftp.gnu.org/gnu/glibc/glibc-%{version}.tar.xz
Source1: https://ftp.gnu.org/gnu/glibc/glibc-%{version}.tar.xz.sig
Source2: gpgkey-7273542B39962DF7B299931416792B4EA25340F8.asc

Source11: glibc-tmpfiles.conf
Source12: ld.so.conf
Source13: ldconfig-service.conf
Source14: tz-utc.txt

# We include this patch as a source file to have more control over how it's
# applied and reverted during the build.
Source99: HACK-only-build-and-install-localedef.patch

# Upstream patches from 2.40 release branch:
# ```
# git checkout origin/release/2.40/master
# git format-patch --no-numbered glibc-2.40..
# ```
Patch0001: 0001-Replace-advisories-directory.patch
Patch0002: 0002-resolv-Allow-short-error-responses-to-match-any-quer.patch
Patch0003: 0003-resolv-Do-not-wait-for-non-existing-second-DNS-respo.patch
Patch0004: 0004-manual-Do-not-mention-STATIC_TLS-in-dynamic-linker-h.patch
Patch0005: 0005-Fix-version-number-in-NEWS-file.patch
Patch0006: 0006-malloc-avoid-global-locks-in-tst-aligned_alloc-lib.c.patch
Patch0007: 0007-malloc-add-multi-threaded-tests-for-aligned_alloc-ca.patch
Patch0008: 0008-manual-stdio-Clarify-putc-and-putwc.patch
Patch0009: 0009-manual-make-setrlimit-description-less-ambiguous.patch
Patch0010: 0010-Enhance-test-coverage-for-strnlen-wcsnlen.patch
Patch0011: 0011-Enhanced-test-coverage-for-strncmp-wcsncmp.patch
Patch0012: 0012-linux-Update-the-mremap-C-implementation-BZ-31968.patch
Patch0013: 0013-mremap-Update-manual-entry.patch
Patch0014: 0014-Add-mremap-tests.patch
Patch0015: 0015-resolv-Fix-tst-resolv-short-response-for-older-GCC-b.patch
Patch0016: 0016-x86-Tunables-may-incorrectly-set-Prefer_PMINUB_for_s.patch
Patch0017: 0017-Fix-name-space-violation-in-fortify-wrappers-bug-320.patch
Patch0018: 0018-manual-stdio-Further-clarify-putc-putwc-getc-and-get.patch
Patch0019: 0019-support-Add-options-list-terminator-to-the-test-driv.patch
Patch0020: 0020-x86-64-Remove-sysdeps-x86_64-x32-dl-machine.h.patch
Patch0021: 0021-x32-cet-Support-shadow-stack-during-startup-for-Linu.patch
Patch0022: 0022-x86-Fix-bug-in-strchrnul-evex512-BZ-32078.patch
Patch0023: 0023-Define-__libc_initial-for-the-static-libc.patch
Patch0024: 0024-string-strerror-strsignal-cannot-use-buffer-after-dl.patch
Patch0025: 0025-support-Add-FAIL-test-failure-helper.patch
Patch0026: 0026-stdio-common-Add-test-for-vfscanf-with-matches-longe.patch
Patch0027: 0027-Make-tst-ungetc-use-libsupport.patch
Patch0028: 0028-ungetc-Fix-uninitialized-read-when-putting-into-unus.patch
Patch0029: 0029-ungetc-Fix-backup-buffer-leak-on-program-exit-BZ-278.patch
Patch0030: 0030-posix-Use-support-check.h-facilities-in-tst-truncate.patch
Patch0031: 0031-nptl-Use-support-check.h-facilities-in-tst-setuid3.patch
Patch0032: 0032-elf-Clarify-and-invert-second-argument-of-_dl_alloca.patch
Patch0033: 0033-elf-Avoid-re-initializing-already-allocated-TLS-in-d.patch
Patch0034: 0034-elf-Fix-tst-dlopen-tlsreinit1.out-test-dependency.patch
Patch0035: 0035-debug-Fix-read-error-handling-in-pcprofiledump.patch
Patch0036: 0036-libio-Attempt-wide-backup-free-only-for-non-legacy-c.patch
Patch0037: 0037-stdio-common-Add-new-test-for-fdopen.patch
Patch0038: 0038-Add-tests-of-fread.patch
Patch0039: 0039-Test-errno-setting-on-strtod-overflow-in-tst-strtod-.patch
Patch0040: 0040-More-thoroughly-test-underflow-errno-in-tst-strtod-r.patch
Patch0041: 0041-Fix-strtod-subnormal-rounding-bug-30220.patch
Patch0042: 0042-Make-__strtod_internal-tests-type-generic.patch
Patch0043: 0043-Improve-NaN-payload-testing.patch
Patch0044: 0044-Do-not-set-errno-for-overflowing-NaN-payload-in-strt.patch
Patch0045: 0045-powerpc64le-Build-new-strtod-tests-with-long-double-.patch
Patch0046: 0046-Make-tst-strtod2-and-tst-strtod5-type-generic.patch
Patch0047: 0047-Add-more-tests-of-strtod-end-pointer.patch
Patch0048: 0048-Add-tests-of-more-strtod-special-cases.patch
Patch0049: 0049-libio-Set-_vtable_offset-before-calling-_IO_link_in-.patch
Patch0050: 0050-Make-tst-strtod-underflow-type-generic.patch
Patch0051: 0051-elf-Change-ldconfig-auxcache-magic-number-bug-32231.patch
Patch0052: 0052-Mitigation-for-clone-on-sparc-might-fail-with-EFAULT.patch
Patch0053: 0053-linux-sparc-Fix-clone-for-LEON-sparcv8-BZ-31394.patch
Patch0054: 0054-elf-handle-addition-overflow-in-_dl_find_object_upda.patch
Patch0055: 0055-nptl-initialize-rseq-area-prior-to-registration.patch
Patch0056: 0056-nptl-initialize-cpu_id_start-prior-to-rseq-registrat.patch
Patch0057: 0057-malloc-add-indirection-for-malloc-like-functions-in-.patch

# Fedora patches
Patch1001: glibc-cs-path.patch

# Local patches
Patch9001: 9001-move-ldconfig-cache-to-ephemeral-storage.patch

%description
%{summary}.

%package devel
Summary: Files for development using the GNU libc libraries.
Requires: %{name}

%description devel
%{summary}.

%prep
%{gpgverify} --data=%{S:0} --signature=%{S:1} --keyring=%{S:2}
%autosetup -Sgit -n glibc-%{version} -p1

%global glibc_configure %{shrink: \
BUILDFLAGS="-O2 -g -Wp,-D_GLIBCXX_ASSERTIONS -fstack-clash-protection" \
CFLAGS="${BUILDFLAGS}" CPPFLAGS="" CXXFLAGS="${BUILDFLAGS}" \
../configure \
  --prefix="%{_cross_prefix}" \
  --sysconfdir="%{_cross_sysconfdir}" \
  --localstatedir="%{_cross_localstatedir}" \
  --enable-bind-now \
  --enable-fortify-source \
  --enable-multi-arch \
  --enable-shared \
  --enable-stack-protector=strong \
  --disable-build-nscd \
  --disable-crypt \
  --disable-nscd \
  --disable-profile \
  --disable-systemtap \
  --disable-timezone-tools \
  --without-gd \
  --without-selinux
  %{nil}}

%build

# First build the host tools we need, namely `localedef`. Apply a patch from
# Buildroot that allows us to build just this program and not everything.
patch -p1 < %{S:99}

mkdir build
pushd build
%glibc_configure
make %{?_smp_mflags} -O -r locale/others
mv locale/localedef %{_builddir}/localedef
popd

# Remove the previous build, revert the patch, and verify that the tree is
# clean, since we don't want to contaminate our target build.
rm -rf build
patch -p1 -R < %{S:99}
git diff --quiet

# Now build for the target. This is what will end up in the package, except
# for the C.UTF-8 locale, which we need `localedef` to generate.
mkdir build
pushd build
%glibc_configure \
  --target="%{_cross_target}" \
  --host="%{_cross_target}" \
  --build="%{_build}" \
  --with-headers="%{_cross_includedir}" \
  --enable-kernel="5.10.0"
make %{?_smp_mflags} -O -r
popd

%install
pushd build
make -j1 install_root=%{buildroot} install
# By default, LOCALEDEF refers to the target binary, and is invoked by the
# dynamic linker that was just built for the target. Neither will run on a
# build host with a different architecture. The locale format is compatible
# across architectures but not across glibc versions, so we can't rely on
# the binary in the SDK and must use the one we built earlier.
make -j1 install_root=%{buildroot} install-files-C.UTF-8/UTF-8 -C ../localedata objdir="$(pwd)" \
  LOCALEDEF="I18NPATH=. GCONV_PATH=$(pwd)/../iconvdata LC_ALL=C %{_builddir}/localedef"
popd

install -d %{buildroot}%{_cross_tmpfilesdir}
install -d %{buildroot}%{_cross_factorydir}%{_cross_sysconfdir}
install -d %{buildroot}%{_cross_unitdir}/ldconfig.service.d

install -p -m 0644 %{S:11} %{buildroot}%{_cross_tmpfilesdir}/glibc.conf
install -p -m 0644 %{S:12} %{buildroot}%{_cross_factorydir}%{_cross_sysconfdir}/ld.so.conf
install -p -m 0644 %{S:13} %{buildroot}%{_cross_unitdir}/ldconfig.service.d/ldconfig.conf

truncate -s 0 %{buildroot}%{_cross_libdir}/gconv/gconv-modules
chmod 644 %{buildroot}%{_cross_libdir}/gconv/gconv-modules
truncate -s 0 %{buildroot}%{_cross_libdir}/gconv/gconv-modules.cache
chmod 644 %{buildroot}%{_cross_libdir}/gconv/gconv-modules.cache

truncate -s 0 %{buildroot}%{_cross_datadir}/locale/locale.alias
chmod 644 %{buildroot}%{_cross_datadir}/locale/locale.alias

install -d %{buildroot}%{_cross_datadir}/zoneinfo
base64 --decode %{S:14} > %{buildroot}%{_cross_datadir}/zoneinfo/UTC

%files
%license COPYING COPYING.LIB LICENSES
%{_cross_attribution_file}
%{_cross_tmpfilesdir}/glibc.conf
%exclude %{_cross_sysconfdir}/rpc

%{_cross_bindir}/getconf
%{_cross_bindir}/getent
%exclude %{_cross_bindir}/gencat
%exclude %{_cross_bindir}/iconv
%exclude %{_cross_bindir}/ld.so
%exclude %{_cross_bindir}/ldd
%exclude %{_cross_bindir}/locale
%exclude %{_cross_bindir}/localedef
%exclude %{_cross_bindir}/makedb
%exclude %{_cross_bindir}/mtrace
%exclude %{_cross_bindir}/pldd
%exclude %{_cross_bindir}/pcprofiledump
%exclude %{_cross_bindir}/sotruss
%exclude %{_cross_bindir}/sprof
%exclude %{_cross_bindir}/xtrace

%{_cross_sbindir}/ldconfig
%exclude %{_cross_sbindir}/iconvconfig
%exclude %{_cross_sbindir}/sln

%dir %{_cross_libexecdir}/getconf
%{_cross_libexecdir}/getconf/*

%{_cross_libdir}/ld-linux-*.so.*
%{_cross_libdir}/libBrokenLocale.so.*
%{_cross_libdir}/libanl.so.*
%{_cross_libdir}/libc.so.*
%{_cross_libdir}/libdl.so.*
%{_cross_libdir}/libm.so.*
%{_cross_libdir}/libnss_dns.so.*
%{_cross_libdir}/libnss_files.so.*
%{_cross_libdir}/libpthread.so.*
%{_cross_libdir}/libresolv.so.*
%{_cross_libdir}/librt.so.*
%{_cross_libdir}/libthread_db.so.*
%{_cross_libdir}/libutil.so.*
%{_cross_libdir}/libmvec.so.*
%exclude %{_cross_libdir}/audit/sotruss-lib.so
%exclude %{_cross_libdir}/libc_malloc_debug.so.*
%exclude %{_cross_libdir}/libmemusage.so
%exclude %{_cross_libdir}/libpcprofile.so
%exclude %{_cross_libdir}/libnsl.so.*
%exclude %{_cross_libdir}/libnss_compat.so.*
%exclude %{_cross_libdir}/libnss_db.so.*
%exclude %{_cross_libdir}/libnss_hesiod.so.*

%dir %{_cross_libdir}/gconv
%dir %{_cross_libdir}/gconv/gconv-modules.d
%{_cross_libdir}/gconv/gconv-modules
%{_cross_libdir}/gconv/gconv-modules.cache
%exclude %{_cross_libdir}/gconv/*.so
%exclude %{_cross_libdir}/gconv/gconv-modules.d/*.conf

%dir %{_cross_libdir}/locale
%dir %{_cross_libdir}/locale/C.utf8
%{_cross_libdir}/locale/C.utf8/LC_*

%dir %{_cross_datadir}/i18n
%dir %{_cross_datadir}/i18n/charmaps
%dir %{_cross_datadir}/i18n/locales
%dir %{_cross_datadir}/locale
%{_cross_datadir}/locale/locale.alias
%dir %{_cross_datadir}/zoneinfo
%{_cross_datadir}/zoneinfo/UTC
%exclude %{_cross_datadir}/i18n/charmaps/*
%exclude %{_cross_datadir}/i18n/locales/*
%exclude %{_cross_datadir}/locale/*
%exclude %{_cross_localstatedir}/db/Makefile

%dir %{_cross_factorydir}
%{_cross_factorydir}%{_cross_sysconfdir}/ld.so.conf

%dir %{_cross_unitdir}/ldconfig.service.d
%{_cross_libdir}/systemd/system/ldconfig.service.d/ldconfig.conf

%files devel
%{_cross_libdir}/*.a
%{_cross_libdir}/*.o
%{_cross_libdir}/libBrokenLocale.so
%{_cross_libdir}/libanl.so
%{_cross_libdir}/libc.so
%{_cross_libdir}/libm.so
%{_cross_libdir}/libresolv.so
%{_cross_libdir}/libthread_db.so
%{_cross_libdir}/libmvec.so
%exclude %{_cross_libdir}/libc_malloc_debug.so
%exclude %{_cross_libdir}/libnss_compat.so
%exclude %{_cross_libdir}/libnss_db.so
%exclude %{_cross_libdir}/libnss_hesiod.so

%dir %{_cross_includedir}/arpa
%dir %{_cross_includedir}/bits
%dir %{_cross_includedir}/gnu
%dir %{_cross_includedir}/net
%dir %{_cross_includedir}/netinet
%dir %{_cross_includedir}/netipx
%dir %{_cross_includedir}/netiucv
%dir %{_cross_includedir}/netpacket
%dir %{_cross_includedir}/netrose
%dir %{_cross_includedir}/nfs
%dir %{_cross_includedir}/protocols
%dir %{_cross_includedir}/rpc
%dir %{_cross_includedir}/scsi
%dir %{_cross_includedir}/sys
%dir %{_cross_includedir}/netash
%dir %{_cross_includedir}/netatalk
%dir %{_cross_includedir}/netax25
%dir %{_cross_includedir}/neteconet
%dir %{_cross_includedir}/netrom
%{_cross_includedir}/*.h
%{_cross_includedir}/*/*

%changelog
