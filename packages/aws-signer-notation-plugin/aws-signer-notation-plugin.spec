%global goproject github.com/aws
%global gorepo aws-signer-notation-plugin
%global goimport %{goproject}/%{gorepo}

%global gover 1.0.2292
%global rpmver %{gover}

Name: %{_cross_os}aws-signer-notation-plugin
Version: %{rpmver}
Release: 1%{?dist}
Summary: AWS Signer plugin for Notation
License: Apache-2.0
URL: https://%{goimport}
Source0: https://%{goimport}/archive/v%{gover}/%{gorepo}-v%{gover}.tar.gz
Source1: bundled-%{gorepo}-v%{gover}.tar.gz
Source2: aws-signer-notation-plugin-tmpfiles.conf
# The commercial and gov root certificates for AWS Signer.
Source3: aws-signer-notation-root.crt
Source4: aws-us-gov-signer-notation-root.crt


BuildRequires: git
BuildRequires: %{_cross_os}glibc-devel
Requires: %{name}(binaries)
Requires: %{_cross_os}notation
Requires: %{_cross_os}ecr-credential-helper

%description
%{summary}.

%package bin
Summary: AWS Signer plugin for Notation binaries
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: AWS Signer plugin for Notation binaries, FIPS edition
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

go build -ldflags "${GOLDFLAGS}" -o notation-com.amazonaws.signer.notation.plugin ./cmd
gofips build -ldflags "${GOLDFLAGS}" -o fips/notation-com.amazonaws.signer.notation.plugin ./cmd

%install
install -d %{buildroot}{%{_cross_bindir},%{_cross_fips_bindir},%{_cross_templatedir}}

# Place the binaries where notation expects them.
# TODO: package these first two dirs in notation package?
install -d %{buildroot}%{_cross_bindir}/notation-plugins
install -d %{buildroot}%{_cross_bindir}/notation-plugins/plugins
install -d %{buildroot}%{_cross_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin

install -d %{buildroot}%{_cross_fips_bindir}/notation-plugins
install -d %{buildroot}%{_cross_fips_bindir}/notation-plugins/plugins
install -d %{buildroot}%{_cross_fips_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin


install -p -m 0755 notation-com.amazonaws.signer.notation.plugin %{buildroot}%{_cross_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin/notation-com.amazonaws.signer.notation.plugin
install -p -m 0755 fips/notation-com.amazonaws.signer.notation.plugin %{buildroot}%{_cross_fips_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin/notation-com.amazonaws.signer.notation.plugin

install -p -m 0644 %{S:3} %{buildroot}%{_cross_templatedir}/aws-signer-notation-root-ca
install -p -m 0644 %{S:4} %{buildroot}%{_cross_templatedir}/aws-us-gov-signer-notation-root-ca

# Add the notation config truststore directories.
install -d %{buildroot}%{_cross_tmpfilesdir}
install -p -m 0644 %{S:2} %{buildroot}%{_cross_tmpfilesdir}/aws-signer-notation-plugin-tmpfiles.conf

%cross_scan_attribution go-vendor vendor

%files
%license LICENSE
%{_cross_attribution_file}
%{_cross_attribution_vendor_dir}
%{_cross_templatedir}/aws-signer-notation-root-ca
%{_cross_templatedir}/aws-us-gov-signer-notation-root-ca

%files bin
%{_cross_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin/notation-com.amazonaws.signer.notation.plugin
%{_cross_tmpfilesdir}/aws-signer-notation-plugin-tmpfiles.conf

%files fips-bin
%{_cross_fips_bindir}/notation-plugins/plugins/com.amazonaws.signer.notation.plugin/notation-com.amazonaws.signer.notation.plugin
%{_cross_tmpfilesdir}/aws-signer-notation-plugin-tmpfiles.conf

%changelog
