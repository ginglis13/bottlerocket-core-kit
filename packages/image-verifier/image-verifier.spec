%global _cross_first_party 1
%global workspace_name image-verifier

Name: %{_cross_os}%{workspace_name}
Version: 0.1.0
Release: 1%{?dist}
Summary: Binary for containerd image verification plugin
License: Apache-2.0 OR MIT
URL: https://github.com/bottlerocket-os/bottlerocket
BuildRequires: %{_cross_os}glibc-devel

%description
%{summary}.

%prep
%setup -T -c
cp -r %{_builddir}/sources/%{workspace_name}/* .

%build
export GO_MAJOR="1.24"

%set_cross_go_flags
go build -ldflags="${GOLDFLAGS}" -o image-verifier .

%install
install -d %{buildroot}%{_cross_bindir}/image-verifier
install -d %{buildroot}%{_cross_bindir}/image-verifier/bin
install -p -m 0755 image-verifier %{buildroot}%{_cross_bindir}/image-verifier/bin

# TODO: no dependencies to vendor
# %%cross_scan_attribution go-vendor vendor

%files
%dir %{_cross_bindir}/image-verifier
%dir %{_cross_bindir}/image-verifier/bin
%{_cross_bindir}/image-verifier/bin/image-verifier

%changelog
