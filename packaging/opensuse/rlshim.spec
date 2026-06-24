Name:           rlshim
Version:        1.1.0
Release:        1%{?dist}
Summary:        A lightweight, native Linux launcher for RuneLite.

License:        BSD-2-Clause
URL:            https://github.com/RdrSeraphim/rlshim
Source0:        %{name}-%{version}.tar.gz
Source1:        libcurl-impersonate-v1.5.6.x86_64-linux-gnu.tar.gz
Source2:        json-3.12.0.tar.gz
Source3:        glfw-3.3.8.tar.gz
Source4:        imgui-1.92.8.tar.gz
Source5:        libcurl-impersonate-v1.5.6.aarch64-linux-gnu.tar.gz

# Leap 15.6 and 16.0 toolchains lack the C++23 <print> header
%if 0%{?suse_version} <= 1600
Patch0:         fix-print-header.patch
%endif

BuildRequires:  cmake
BuildRequires:  clang
%if 0%{?suse_version} < 1600
BuildRequires:  gcc13-c++
%else
BuildRequires:  gcc-c++
%endif
BuildRequires:  make
BuildRequires:  pkg-config
BuildRequires:  pkgconf
BuildRequires:  libsecret-devel
BuildRequires:  openssl-devel
BuildRequires:  libglfw-devel
BuildRequires:  Mesa-libGL-devel
BuildRequires:  libcurl-devel
BuildRequires:  libX11-devel
BuildRequires:  libXcursor-devel
BuildRequires:  libXi-devel
BuildRequires:  libXinerama-devel
BuildRequires:  libXrandr-devel
%if 0%{?suse_version} != 1600
BuildRequires:  lld
%else
BuildRequires:  llvm-gold
%endif
BuildRequires:  git

Requires:       libsecret
Requires:       openssl
Requires:       glfw
Requires:       glibc

%description
rlshim is a lightweight, native Linux launcher for RuneLite, designed as an alternative to the official AppImage and Flatpak.

%prep
%setup -q -n rlshim-%{version} -a 2 -a 3 -a 4
%if 0%{?suse_version} <= 1600
sed -i '/CheckIPOSupported/d' CMakeLists.txt
sed -i '/check_ipo_supported/d' CMakeLists.txt
perl -pi -e 's/\Qcmake_policy(SET CMP0169 OLD)\E/if(POLICY CMP0169)\n    cmake_policy(SET CMP0169 OLD)\nendif()/g' CMakeLists.txt
%patch -P 0 -p1
%endif

%build
mkdir -p build/_deps/curl-impersonate/extracted
%ifarch x86_64
tar -xzf %{SOURCE1} -C build/_deps/curl-impersonate/extracted
%endif
%ifarch aarch64
tar -xzf %{SOURCE5} -C build/_deps/curl-impersonate/extracted
%endif

if command -v clang++ &> /dev/null; then
    export CC=clang
    export CXX=clang++
else
    for ver in 17 18 19 20 21 22 23 24; do
        if command -v clang++-$ver &> /dev/null; then
            export CC=clang-$ver
            export CXX=clang++-$ver
            break
        fi
    done
fi

mkdir -p local_bin
if command -v lld &> /dev/null; then
    ln -sf $(command -v lld) local_bin/ld.lld
else
    for ver in 17 18 19 20 21 22 23 24; do
        if command -v lld-$ver &> /dev/null; then
            ln -sf $(command -v lld-$ver) local_bin/ld.lld
            break
        fi
    done
fi
export PATH="$PWD/local_bin:$PATH"

GCC_DIR_FLAG=""
if ls -1d /usr/lib64/gcc/*-suse-linux/13* >/dev/null 2>&1; then
    GCC_DIR=$(ls -1d /usr/lib64/gcc/*-suse-linux/13* | head -n 1)
    GCC_DIR_FLAG="--gcc-install-dir=$GCC_DIR"
fi

if command -v lld &> /dev/null || command -v ld.lld &> /dev/null; then
    USE_LLD="-fuse-ld=lld"
else
    USE_LLD=""
fi

export LDFLAGS="%{?__global_ldflags} $USE_LLD $GCC_DIR_FLAG"
export CFLAGS="%{optflags} $USE_LLD $GCC_DIR_FLAG"
export CXXFLAGS="%{optflags} $USE_LLD $GCC_DIR_FLAG"

%cmake -DCMAKE_BUILD_TYPE=Release \
       -DFETCHCONTENT_SOURCE_DIR_JSON=%{_builddir}/rlshim-%{version}/json-3.12.0 \
       -DFETCHCONTENT_SOURCE_DIR_GLFW=%{_builddir}/rlshim-%{version}/glfw-3.3.8 \
       -DFETCHCONTENT_SOURCE_DIR_IMGUI=%{_builddir}/rlshim-%{version}/imgui-1.92.8
%cmake_build

%install
%cmake_install

%files
%license LICENSE
%{_bindir}/rlshim
%{_datadir}/applications/rlshim.desktop
%{_datadir}/pixmaps/rlshim.png
%dir %{_datadir}/rlshim
%dir %{_datadir}/rlshim/data
%{_datadir}/rlshim/data/background.png
%{_datadir}/rlshim/data/icon.png
%{_datadir}/rlshim/data/*.ttf

%changelog
* Wed Jun 24 2026 Seraphim Pardee <me@srp.life> - 1.1.0-1
- Add flatpak support.
* Tue Jun 23 2026 Seraphim Pardee <me@srp.life> - 1.0.1-1
- Add placeholders for empty character names in character selection menu.
* Sun Jun 14 2026 Seraphim Pardee <me@srp.life> - 1.0.0-1
- Initial release
