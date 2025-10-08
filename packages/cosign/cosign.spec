%global goproject github.com/sigstore
%global gorepo cosign
%global goimport %{goproject}/%{gorepo}

%global gover 3.0.2
%global rpmver %{gover}

%global _dwz_low_mem_die_limit 0

Name: %{_cross_os}cosign
Version: %{rpmver}
Release: 1%{?dist}
Summary: Container signing and verification tool
License: Apache-2.0
URL: https://github.com/sigstore/cosign

Source: cosign-v%{gover}.tar.gz
Source1: bundled-cosign-v%{gover}.tar.gz

BuildRequires: %{_cross_os}glibc-devel
Requires: %{name}(binaries)

%description
%{summary}.

%package bin
Summary: Cosign binaries
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: Cosign binaries, FIPS edition
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

go build -ldflags "${GOLDFLAGS}" -o cosign ./cmd/cosign
gofips build -ldflags "${GOLDFLAGS}" -o fips/cosign ./cmd/cosign

%install
install -d %{buildroot}%{_cross_bindir}
install -p -m 0755 cosign %{buildroot}%{_cross_bindir}/cosign

install -d %{buildroot}%{_cross_fips_bindir}
install -p -m 0755 fips/cosign %{buildroot}%{_cross_fips_bindir}/cosign

%cross_scan_attribution go-vendor vendor

%files
%license LICENSE
%{_cross_attribution_file}
%{_cross_attribution_vendor_dir}

%files bin
%{_cross_bindir}/cosign

%files fips-bin
%{_cross_fips_bindir}/cosign
