Name: %{_cross_os}libnftnl
Version: 1.2.8
Release: 1%{?dist}
Epoch: 1
Summary: Library for nftables netlink
License: GPL-2.0-or-later AND GPL-2.0-only
URL: http://netfilter.org/projects/libnftnl/
Source0: https://netfilter.org/projects/libnftnl/files/libnftnl-%{version}.tar.xz
Source1: https://netfilter.org/projects/libnftnl/files/libnftnl-%{version}.tar.xz.sig
Source2: gpgkey-37D964ACC04981C75500FB9BD55D978A8A1420E4.asc
BuildRequires: %{_cross_os}glibc-devel
BuildRequires: %{_cross_os}libmnl-devel
Requires: %{_cross_os}libmnl

%description
%{summary}.

%package devel
Summary: Files for development using the library for nftables netlink
Requires: %{name}

%description devel
%{summary}.

%prep
%{gpgverify} --data=%{S:0} --signature=%{S:1} --keyring=%{S:2}
%autosetup -n libnftnl-%{version} -p1

%build
%cross_configure \
  --disable-silent-rules \
  --enable-static \
  --without-json-parsing \

%make_build

%install
%make_install

%files
%license COPYING
%{_cross_attribution_file}
%{_cross_libdir}/*.so.*

%files devel
%{_cross_libdir}/*.a
%{_cross_libdir}/*.so
%dir %{_cross_includedir}/libnftnl
%{_cross_includedir}/libnftnl/*.h
%{_cross_pkgconfigdir}/*.pc
%exclude %{_cross_libdir}/*.la

%changelog
