%global goproject github.com/awslabs
%global gorepo amazon-ecr-credential-helper
%global goimport %{goproject}/%{gorepo}

%global gover 0.10.1
%global rpmver %{gover}

%global _dwz_low_mem_die_limit 0

Name: %{_cross_os}ecr-credential-helper
Version: %{rpmver}
Release: 1%{?dist}
Summary: Amazon ECR credential helper
License: Apache-2.0
URL: https://%{goimport}
Source0: https://%{goimport}/archive/v%{gover}/%{gorepo}-%{gover}.tar.gz
Source1: bundled-%{gorepo}-%{gover}.tar.gz
Source2: docker-root-config-json
Source1000: clarify.toml

BuildRequires: %{_cross_os}glibc-devel
Requires: %{name}(binaries)

%description
%{summary}.

%package bin
Summary: Amazon ECR credential helper binaries
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: Amazon ECR credential helper binaries, FIPS edition
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(fips) and %{name})
Conflicts: (%{_cross_os}image-feature(no-fips) or %{name}-bin)

%description fips-bin
%{summary}.

%prep
%autosetup -n %{gorepo}-%{gover} -p1
%cross_go_setup %{gorepo}-%{gover} %{goproject} %{goimport}


%build
##%%set_cross_go_flags
# cross_go_configure cd's to the correct GOPATH location
%cross_go_configure %{goimport}

export GO_MAJOR="1.24"

go build -ldflags="${GOLDFLAGS}" -o=docker-credential-ecr-login ./ecr-login/cli/docker-credential-ecr-login
gofips build -ldflags="${GOLDFLAGS}" -o=fips/docker-credential-ecr-login ./ecr-login/cli/docker-credential-ecr-login

%install
install -d %{buildroot}%{_cross_bindir}
install -p -m 0755 docker-credential-ecr-login %{buildroot}%{_cross_bindir}

install -d %{buildroot}%{_cross_fips_bindir}
install -p -m 0755 fips/docker-credential-ecr-login %{buildroot}%{_cross_fips_bindir}

%cross_scan_attribution --clarify %{S:1000} go-vendor ./ecr-login/vendor

## TODO: similar approach for the root CAs in the aws-signer-notation-plugin package.
install -d %{buildroot}/root/.docker
install -m 0644 %{S:2} %{buildroot}/root/.docker/config.json

%files
%license LICENSE
%{_cross_attribution_file}
%{_cross_attribution_vendor_dir}
%dir /root/.docker
/root/.docker/config.json

%files bin
%{_cross_bindir}/docker-credential-ecr-login

%files fips-bin
%{_cross_fips_bindir}/docker-credential-ecr-login
