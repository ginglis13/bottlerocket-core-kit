%global goproject github.com/notaryproject
%global gorepo notation
%global goimport %{goproject}/%{gorepo}

%global gover 1.3.2
%global rpmver %{gover}
%global gitrev 001cc919603c1dc16c6aad387c94b4209cb9c901

Name: %{_cross_os}%{gorepo}
Version: %{rpmver}
Release: 1%{?dist}
Summary:  A CLI tool to sign and verify artifacts.
License: Apache-2.0
URL: https://%{goimport}
Source0: https://%{goimport}/archive/v%{gover}/%{gorepo}-v%{gover}.tar.gz
Source1: bundled-%{gorepo}-v%{gover}.tar.gz
Source2: notation-trust-policy-json
#Source3: notation-verifier
Source4: notation-tmpfiles.conf

BuildRequires: git
BuildRequires: %{_cross_os}glibc-devel
Requires: %{name}(binaries)

Provides: %{_cross_os}notation

%description
%{summary}.

%package bin
Summary: A CLI tool to sign and verify artifacts' binaries.
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: A CLI tool to sign and verify artifacts' binaries, FIPS edition.
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(fips) and %{name})
Conflicts: (%{_cross_os}image-feature(no-fips) or %{name}-bin)

%description fips-bin
%{summary}.

%prep
%setup -n %{gorepo}-%{gover} -q
%setup -T -D -n %{gorepo}-%{gover} -b 1 -q

%build

%set_cross_go_flags

go build -ldflags "${GOLDFLAGS}" -o notation ./cmd/notation
gofips build -ldflags "${GOLDFLAGS}" -o fips/notation ./cmd/notation

%install
install -d %{buildroot}{%{_cross_bindir},%{_cross_fips_bindir},%{_cross_templatedir}}

install -p -m 0755 notation %{buildroot}%{_cross_bindir}
install -p -m 0755 fips/notation %{buildroot}%{_cross_fips_bindir}
install -p -m 0644 %{S:2} %{buildroot}%{_cross_templatedir}/notation-trust-policy-json

# Add the notation config and cache directories
install -d %{buildroot}%{_cross_tmpfilesdir}
install -p -m 0644 %{S:4} %{buildroot}%{_cross_tmpfilesdir}/notation.conf

%cross_scan_attribution go-vendor vendor

%files
%license LICENSE
%{_cross_attribution_file}
%{_cross_attribution_vendor_dir}
%{_cross_templatedir}/notation-trust-policy-json
#%%{_cross_bindir}/image-verifier/bin/notation-verifier
%{_cross_tmpfilesdir}/notation.conf


%files bin
%{_cross_bindir}/notation

%files fips-bin
%{_cross_fips_bindir}/notation

%changelog
