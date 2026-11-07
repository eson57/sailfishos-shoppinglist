%{!?project_root:%{error:build this package with `just rpm`}}
%{!?app_name:%{error:build this package with `just rpm`}}
%{!?org_name:%{error:build this package with `just rpm`}}
%{!?profile_dir:%{error:build this package with `just rpm`}}

%global _prefix /usr
%global _exec_prefix /usr
%global _bindir /usr/bin
%global _datadir /usr/share
%global _libdir /usr/lib64

%define datadir %{_datadir}/%{name}

%global debug_package %{nil}
%global _binary_payload w3.zstdio
%global __os_install_post %{nil}

Name:       %{app_name}
Version:    %{?ver}%{!?ver:0.1.0}
Release:    1
Summary:    Shopping list
License:    GPLv3
URL:        https://github.com/simmsb/sailfishos-shoppinglist
Group:      Qt/Qt

Requires:   sailfishsilica-qt5 >= 0.10.9
Requires:   qr-filter-qml-plugin
Requires:   nemo-qml-plugin-contextkit-qt5
Requires:   nemo-qml-plugin-configuration-qt5
Requires:   sailfishshare-components
Requires:   qt5-qtdeclarative-import-folderlistmodel

# Otherwise on macos we get a Requires: group(root) for some reason
%global _use_weak_usergroup_deps 1

%description
A Sailfish OS shopping list.

%prep

%build

%install
rm -rf %{buildroot}
install -Dm 755 %{project_root}/target/aarch64-unknown-linux-gnu/%{profile_dir}/%{name} \
    %{buildroot}%{_bindir}/%{name}

# SailfishApp::pathTo resolves paths under %{_datadir}/%{name}.
find %{project_root}/qml -type f | while read -r f; do
    rel=${f#%{project_root}/qml/}
    install -Dm 644 "$f" "%{buildroot}%{datadir}/qml/$rel"
done

: > %{_builddir}/locale.files
if [ -d %{project_root}/target/install/share/locale ]; then
    find %{project_root}/target/install/share/locale -type f -name '*.mo' | while read -r f; do
        rel=${f#%{project_root}/target/install/}
        install -Dm 644 "$f" "%{buildroot}%{_datadir}/${rel#share/}"
        echo "%{_datadir}/${rel#share/}" >> %{_builddir}/locale.files
    done
fi

for size in 86 108 128 172; do
    mkdir -p %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps
    rsvg-convert -w $size -h $size \
        %{project_root}/icons/sailfishos-shoppinglist.svg \
        -o %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/%{name}.png
done

mkdir -p %{buildroot}%{datadir}/qml/images
rsvg-convert -w 256 -h 256 \
    %{project_root}/icons/sailfishos-shoppinglist.svg \
    -o %{buildroot}%{datadir}/qml/images/%{name}.png

mkdir -p %{buildroot}%{_datadir}/applications
sed -e 's/@APPLICATION@/%{app_name}/g' -e 's/@ORGANIZATION@/%{org_name}/g' \
    %{project_root}/rpm/app.desktop.in \
    > %{buildroot}%{_datadir}/applications/%{name}.desktop

%post
update-desktop-database %{_datadir}/applications >/dev/null 2>&1 || :

%postun
update-desktop-database %{_datadir}/applications >/dev/null 2>&1 || :

%files -f %{_builddir}/locale.files
%defattr(-,root,root,-)
%{_bindir}/%{name}
%{datadir}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png
