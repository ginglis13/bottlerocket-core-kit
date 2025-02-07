%global gorepo soci-snapshotter
%global gover 0.9.0
%global rpmver %{gover}
%global gitrev 737f61a3db40c386f997c1f126344158aa3ad43c

Name: %{_cross_os}soci-snapshotter
Version: %{gover}
Release: 1%{?dist}
Epoch: 1
Summary: A containerd snapshotter plugin which enables lazy loading for OCI images.
License: Apache-2.0
URL: https://github.com/awslabs/soci-snapshotter
Source0: https://github.com/awslabs/soci-snapshotter/archive/refs/tags/v%{gover}.tar.gz
Source1: bundled-v%{gover}.tar.gz
Source2: bundled-cmd.tar.gz
Source3: soci-config-toml
# Mount for writing soci configuration
Source100: etc-soci-snapshotter-grpc.mount
Source101: soci-snapshotter.service
Source1000: clarify.toml

BuildRequires: %{_cross_os}glibc-devel
BuildRequires: %{_cross_os}libz-devel
Requires: %{name}(binaries)

%description
%{summary}.

%package bin
Summary: Remote management agent binaries
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: Remote management agent binaries, FIPS edition
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(fips) and %{name})
Conflicts: (%{_cross_os}image-feature(no-fips) or %{name}-bin)

%description fips-bin
%{summary}.

%package cli-bin
Summary: soci CLI binary,
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description cli-bin
%{summary}.

%package cli-fips-bin
Summary: soci CLI binary, FIPS edition
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(fips) and %{name})
Conflicts: (%{_cross_os}image-feature(no-fips) or %{name}-bin)

%description cli-fips-bin
%{summary}.


%prep
%setup -n %{gorepo}-%{gover} -q
%setup -T -D -n %{gorepo}-%{gover} -b 1 -q
%setup -T -D -n %{gorepo}-%{gover} -b 2 -q

%build
%set_cross_go_flags

export LD_VERSION="-X github.com/awslabs/soci-snapshotter/version.Version=v%{gover}+bottlerocket"
export LD_REVISION="-X github.com/awslabs/soci-snapshotter/version.Revision=%{gitrev}"

go build -C cmd -ldflags="${GOLDFLAGS} ${LD_VERSION} ${LD_REVISION}" -o "../out/soci-snapshotter-grpc" ./soci-snapshotter-grpc
go build -C cmd -ldflags="${GOLDFLAGS} ${LD_VERSION} ${LD_REVISION}" -o "../out/soci" ./soci

gofips build -C cmd -ldflags="${GOLDFLAGS} ${LD_VERSION} ${LD_REVISION}" -o "../out/fips/soci-snapshotter-grpc" ./soci-snapshotter-grpc
gofips build -C cmd -ldflags="${GOLDFLAGS} ${LD_VERSION} ${LD_REVISION}" -o "../out/fips/soci" ./soci

%install
install -d %{buildroot}%{_cross_bindir}
install -d %{buildroot}%{_cross_fips_bindir}
install -d %{buildroot}%{_cross_unitdir}
install -p -m 0755 out/soci-snapshotter-grpc %{buildroot}%{_cross_bindir}
install -p -m 0755 out/soci %{buildroot}%{_cross_bindir}
install -p -m 0755 out/fips/soci-snapshotter-grpc %{buildroot}%{_cross_fips_bindir}
install -p -m 0755 out/fips/soci %{buildroot}%{_cross_fips_bindir}
install -D -p -m 0644 %{S:100} %{S:101} %{buildroot}%{_cross_unitdir}

# Install SOCI config
install -d %{buildroot}%{_cross_templatedir}
install -d %{buildroot}%{_cross_factorydir}%{_cross_sysconfdir}/soci-snapshotter-grpc
install -p -m 0644 %{S:3} %{buildroot}%{_cross_templatedir}

%cross_scan_attribution --clarify %{S:1000} go-vendor vendor

%files
%license LICENSE NOTICE.md
%{_cross_attribution_file}
%{_cross_attribution_vendor_dir}
%{_cross_unitdir}/soci-snapshotter.service
%{_cross_unitdir}/etc-soci-snapshotter-grpc.mount
%dir %{_cross_factorydir}%{_cross_sysconfdir}/soci-snapshotter-grpc
%{_cross_templatedir}/soci-config-toml

%files bin
%{_cross_bindir}/soci-snapshotter-grpc

%files fips-bin
%{_cross_fips_bindir}/soci-snapshotter-grpc

%files cli-bin
%{_cross_bindir}/soci

%files cli-fips-bin
%{_cross_fips_bindir}/soci

%changelog
