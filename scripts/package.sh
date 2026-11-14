#!/usr/bin/env bash
# Build installable packages from the pinned release set.
#
# The release set names the exact component archives and checksums. This script
# never builds a component; it downloads (or reuses) those bytes, stages one
# payload, and renders every requested package format from that payload.
set -Eeuo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BOM=$ROOT/packaging/release-set.json
OUT=$ROOT/dist/packages
FORMATS=deb,rpm,arch,aur,pkg,dmg
ARTIFACTS_DIR=
TARGETS=()
DRY_RUN=0
KEEP_WORK=0
SIGN_IDENTITY=
NFPM=${NFPM:-nfpm}
WORK=

usage() {
    cat <<'USAGE'
usage: scripts/package.sh [options]

Builds .deb, .rpm, Arch, AUR, .pkg and .dmg artifacts from a release set.
Components are taken only from the pinned archives in packaging/release-set.json;
their checksums are verified before anything is staged.

Options:
  --release-set PATH   release set (default packaging/release-set.json)
  --formats LIST       deb,rpm,arch,aur,pkg,dmg (default all)
  --target TRIPLE      limit to one target; repeatable
  --artifacts-dir DIR  prefer local archives by basename over downloading
  --out DIR            output directory (default dist/packages)
  --sign-identity ID   Developer ID Installer identity for .pkg/.dmg
  --dry-run            print the plan; do not download or write packages
  --keep-work          keep the staging directory for inspection
  -h, --help           this text

Requires:
  nfpm            for deb/rpm/arch       (go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest)
  pkgbuild,
  productbuild,
  hdiutil         for macOS pkg/dmg     (Xcode command line tools)
  python3         for reading the release set
USAGE
}

say() { printf '==> %s\n' "$*" >&2; }
note() { printf '    %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --release-set) [ $# -ge 2 ] || die "--release-set needs a path"; BOM=$2; shift 2 ;;
            --formats) [ $# -ge 2 ] || die "--formats needs a list"; FORMATS=$2; shift 2 ;;
            --target) [ $# -ge 2 ] || die "--target needs a triple"; TARGETS+=("$2"); shift 2 ;;
            --artifacts-dir) [ $# -ge 2 ] || die "--artifacts-dir needs a directory"; ARTIFACTS_DIR=$2; shift 2 ;;
            --out) [ $# -ge 2 ] || die "--out needs a directory"; OUT=$2; shift 2 ;;
            --sign-identity) [ $# -ge 2 ] || die "--sign-identity needs a name"; SIGN_IDENTITY=$2; shift 2 ;;
            --dry-run) DRY_RUN=1; shift ;;
            --keep-work) KEEP_WORK=1; shift ;;
            -h|--help) usage; exit 0 ;;
            *) die "unknown option: $1 (try --help)" ;;
        esac
    done
}

bom() { python3 "$ROOT/packaging/bom.py" "$@"; }

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

target_os() {
    case "$1" in
        *-apple-darwin) printf 'darwin' ;;
        *-linux-*) printf 'linux' ;;
        *) return 1 ;;
    esac
}

target_deb_arch() {
    case "$1" in
        x86_64-*) printf 'amd64' ;;
        aarch64-*|arm64-*) printf 'arm64' ;;
        armv7-*) printf 'armv7hl' ;;
        *) return 1 ;;
    esac
}

target_native_arch() {
    case "$1" in
        x86_64-*) printf 'x86_64' ;;
        aarch64-*|arm64-*) printf 'arm64' ;;
        *) return 1 ;;
    esac
}

target_rpm_arch() {
    case "$1" in
        x86_64-*) printf 'x86_64' ;;
        aarch64-*|arm64-*) printf 'aarch64' ;;
        armv7-*) printf 'armv7hl' ;;
        *) return 1 ;;
    esac
}

format_applies() { # format os
    case "$1:$2" in
        deb:linux|rpm:linux|arch:linux|aur:linux|pkg:darwin|dmg:darwin) return 0 ;;
        *) return 1 ;;
    esac
}

fetch_archive() { # url archive sha256
    url=$1
    archive=$2
    expected=$3
    dest=$WORK/artifacts/$archive
    mkdir -p "$WORK/artifacts"
    if [ -n "$ARTIFACTS_DIR" ] && [ -f "$ARTIFACTS_DIR/$archive" ]; then
        cp "$ARTIFACTS_DIR/$archive" "$dest"
        note "using local archive $ARTIFACTS_DIR/$archive"
    else
        case "$url" in
            file://*) cp "${url#file://}" "$dest" ;;
            http://*|https://*) curl -fsSL "$url" -o "$dest" ;;
            *) die "unsupported archive URL: $url" ;;
        esac
    fi
    actual=$(sha256_file "$dest")
    [ "$actual" = "$expected" ] || die "checksum mismatch for $archive (expected $expected, got $actual)"
    printf '%s' "$dest"
}

stage_target() { # target
    target=$1
    os=$(target_os "$target") || die "unknown target: $target"
    stage=$WORK/stage/$target
    rm -rf "$stage"
    case "$os" in
        linux) bin_dir=/usr/bin; doc_dir=/usr/share/doc/$name ;;
        darwin) bin_dir=/usr/local/bin; doc_dir=/usr/local/share/doc/$name ;;
    esac
    mkdir -p "$stage$bin_dir" "$stage$doc_dir"
    while IFS=$'\t' read -r component version archive url sha; do
        [ -n "$component" ] || continue
        say "staging $component $version for $target"
        path=$(fetch_archive "$url" "$archive" "$sha")
        src=$WORK/src/$target/$component
        mkdir -p "$src"
        tar -xzf "$path" -C "$src"
        while IFS= read -r binary; do
            [ -n "$binary" ] || continue
            [ -f "$src/$binary" ] || die "$archive is missing $binary"
            install -m 0755 "$src/$binary" "$stage$bin_dir/$binary"
        done < <(bom binaries "$BOM" "$component")
        while IFS= read -r license; do
            [ -n "$license" ] || continue
            [ -f "$src/$license" ] || die "$archive is missing $license"
            install -m 0644 "$src/$license" "$stage$doc_dir/$license"
        done < <(bom licenses "$BOM" "$component")
    done < <(bom components "$BOM" "$target")
    [ -x "$stage$bin_dir/$name" ] || die "release set did not install $name"
    printf '%s' "$stage"
}

render_nfpm() { # target stage
    target=$1
    stage=$2
    arch=$(target_deb_arch "$target") || die "no package arch for $target"
    cfg=$WORK/nfpm/$target.yaml
    mkdir -p "$(dirname "$cfg")"
    {
        printf 'name: %s\n' "$name"
        printf 'arch: %s\n' "$arch"
        printf 'platform: linux\n'
        printf 'version: %s\n' "$version"
        printf 'section: net\n'
        printf 'priority: optional\n'
        printf 'maintainer: %s\n' "$maintainer"
        printf 'description: "%s"\n' "$description"
        printf 'vendor: %s\n' "$vendor"
        printf 'homepage: %s\n' "$homepage"
        printf 'license: %s\n' "$license"
        printf 'contents:\n'
    } >"$cfg"
    while IFS= read -r binary; do
        [ -n "$binary" ] || continue
        printf '  - src: %s\n    dst: /usr/bin/%s\n    file_info:\n      mode: 0755\n' \
            "$stage/usr/bin/$binary" "$binary" >>"$cfg"
    done < <(all_binaries "$target")
    while IFS= read -r license_file; do
        [ -n "$license_file" ] || continue
        printf '  - src: %s\n    dst: /usr/share/doc/%s/%s\n    file_info:\n      mode: 0644\n' \
            "$stage/usr/share/doc/$name/$license_file" "$name" "$license_file" >>"$cfg"
    done < <(all_licenses "$target")
    printf '%s' "$cfg"
}

all_binaries() { # target
    while IFS=$'\t' read -r component _version _archive _url _sha; do
        [ -n "$component" ] || continue
        bom binaries "$BOM" "$component"
    done < <(bom components "$BOM" "$1")
}

all_licenses() { # target
    while IFS=$'\t' read -r component _version _archive _url _sha; do
        [ -n "$component" ] || continue
        bom licenses "$BOM" "$component"
    done < <(bom components "$BOM" "$1")
}

build_nfpm() { # target stage packager
    target=$1
    stage=$2
    packager=$3
    cfg=$(render_nfpm "$target" "$stage")
    case "$packager" in
        deb)
            arch=$(target_deb_arch "$target") || die "no package arch for $target"
            output=$OUT/${name}_${version}_${arch}.deb
            ;;
        rpm)
            arch=$(target_rpm_arch "$target") || die "no package arch for $target"
            output=$OUT/${name}-${version}-1.${arch}.rpm
            ;;
        archlinux)
            arch=$(target_rpm_arch "$target") || die "no package arch for $target"
            output=$OUT/${name}-${version}-1-${arch}.pkg.tar.zst
            ;;
        *) die "unknown nfpm packager: $packager" ;;
    esac
    say "building $packager package for $target"
    mkdir -p "$OUT"
    "$NFPM" package --config "$cfg" --packager "$packager" --target "$output"
    [ -f "$output" ] || die "nfpm did not write $output"
    printf '%s' "$output"
}

build_pkg() { # target stage
    target=$1
    stage=$2
    [ "$(uname -s)" = Darwin ] || die "pkg/dmg packaging needs macOS"
    native=$(target_native_arch "$target") || die "no macOS arch for $target"
    id=dev.fungos.$name
    pkgdir=$WORK/pkg/$target
    rm -rf "$pkgdir"
    mkdir -p "$pkgdir"
    # install(1) preserves extended attributes; pkgbuild would turn those into
    # AppleDouble ._ entries in the payload.
    find "$stage" -name '._*' -delete
    xattr -cr "$stage" 2>/dev/null || true
    component=$pkgdir/$name.pkg
    COPYFILE_DISABLE=1 pkgbuild --root "$stage" --identifier "$id" --version "$version" \
        --install-location / "$component"
    title="$name $version"
    dist=$pkgdir/Distribution.xml
    sed -e "s|@TITLE@|$title|g" \
        -e "s|@IDENTIFIER@|$id|g" \
        -e "s|@VERSION@|$version|g" \
        -e "s|@COMPONENT@|$name.pkg|g" \
        "$ROOT/packaging/macos/Distribution.xml.in" >"$dist"
    output=$OUT/$name-$version-$native.pkg
    mkdir -p "$OUT"
    productbuild --distribution "$dist" --package-path "$pkgdir" "$output"
    if [ -n "$SIGN_IDENTITY" ]; then
        signed=$output.signed
        productsign --sign "$SIGN_IDENTITY" "$output" "$signed"
        mv "$signed" "$output"
    fi
    printf '%s' "$output"
}

build_dmg() { # target pkg
    target=$1
    pkg=$2
    native=$(target_native_arch "$target") || die "no macOS arch for $target"
    dmgdir=$WORK/dmg/$target
    rm -rf "$dmgdir"
    mkdir -p "$dmgdir"
    cp "$pkg" "$dmgdir/"
    cat >"$dmgdir/README.txt" <<EOF
$name $version

Open $name-$version-$native.pkg to install the Mycelium command line tools
and services. After installation run:

  mycelium setup --gateway https://sso.fpl.dev

or redeem a pairing code with --claim.
EOF
    dmgin=$OUT/$name-$version-$native.dmg
    hdiutil create -quiet -volname "$name $version" -srcfolder "$dmgdir" \
        -ov -format UDZO "$dmgin"
    printf '%s' "$dmgin"
}

build_aur() {
    local targets=("$@")
    [ ${#targets[@]} -gt 0 ] || return 0
    aur=$OUT/aur/$name-bin
    mkdir -p "$aur"
    pkgbuild=$aur/PKGBUILD
    {
        printf '# Generated from packaging/release-set.json by scripts/package.sh.\n'
        printf 'pkgname=%s-bin\n' "$name"
        printf 'pkgver=%s\n' "$version"
        printf 'pkgrel=1\n'
        printf 'pkgdesc="%s"\n' "$description"
        printf 'arch=(' 
        for target in "${targets[@]}"; do
            case "$target" in
                x86_64-*) printf "'x86_64' " ;;
                aarch64-*) printf "'aarch64' " ;;
            esac
        done
        printf ')\n'
        printf 'url="%s"\n' "$homepage"
        printf 'license=(' 
        IFS=' ' read -r -a license_parts <<<"$license"
        for part in "${license_parts[@]}"; do
            [ "$part" = OR ] && continue
            printf "'%s' " "$part"
        done
        printf ')\n'
        printf "provides=('%s')\n" "$name"
        printf "conflicts=('%s')\n" "$name"
        printf 'options=(!strip)\n'
        for target in "${targets[@]}"; do
            case "$target" in
                x86_64-*) arch=x86_64 ;;
                aarch64-*) arch=aarch64 ;;
                *) continue ;;
            esac
            IFS=$'\t' read -r component _version archive url sha < <(bom components "$BOM" "$target")
            printf 'source_%s=("%s")\n' "$arch" "$url"
            printf "sha256sums_%s=('%s')\n" "$arch" "$sha"
        done
        printf '\npackage() {\n'
        while IFS= read -r binary; do
            [ -n "$binary" ] || continue
            printf '  install -Dm755 "$srcdir/%s" "$pkgdir/usr/bin/%s"\n' "$binary" "$binary"
        done < <(all_binaries "${targets[0]}")
        while IFS= read -r license_file; do
            [ -n "$license_file" ] || continue
            printf '  install -Dm644 "$srcdir/%s" "$pkgdir/usr/share/licenses/%s-bin/%s"\n' \
                "$license_file" "$name" "$license_file"
        done < <(all_licenses "${targets[0]}")
        printf '}\n'
    } >"$pkgbuild"
    bash -n "$pkgbuild"
    # AUR expects a committed .SRCINFO next to the recipe.
    {
        printf 'pkgbase = %s-bin\n' "$name"
        printf '\tpkgdesc = %s\n' "$description"
        printf '\tpkgver = %s\n' "$version"
        printf '\tpkgrel = 1\n'
        printf '\turl = %s\n' "$homepage"
        for target in "${targets[@]}"; do
            case "$target" in
                x86_64-*) printf '\tarch = x86_64\n' ;;
                aarch64-*) printf '\tarch = aarch64\n' ;;
            esac
        done
        for part in "${license_parts[@]}"; do
            [ "$part" = OR ] && continue
            printf '\tlicense = %s\n' "$part"
        done
        printf '\tprovides = %s\n' "$name"
        printf '\tconflicts = %s\n' "$name"
        for target in "${targets[@]}"; do
            case "$target" in
                x86_64-*) arch=x86_64 ;;
                aarch64-*) arch=aarch64 ;;
                *) continue ;;
            esac
            IFS=$'\t' read -r component _version archive url sha < <(bom components "$BOM" "$target")
            printf '\tsource_%s = %s\n' "$arch" "$url"
            printf '\tsha256sums_%s = %s\n' "$arch" "$sha"
        done
        printf '\n'
        printf 'pkgname = %s-bin\n' "$name"
    } >"$aur/.SRCINFO"
    printf '%s' "$pkgbuild"
}

write_checksums() {
    [ -d "$OUT" ] || return 0
    (
        cd "$OUT"
        find . -type f ! -name SHA256SUMS -print | sort | while IFS= read -r file; do
            printf '%s  %s\n' "$(sha256_file "$file")" "${file#./}"
        done
    ) >"$OUT/SHA256SUMS"
    note "wrote $OUT/SHA256SUMS"
}

main() {
    parse_args "$@"
    [ -f "$BOM" ] || die "release set not found: $BOM"
    name=$(bom field "$BOM" package.name)
    version=$(bom field "$BOM" release)
    vendor=$(bom field "$BOM" package.vendor)
    homepage=$(bom field "$BOM" package.homepage)
    maintainer=$(bom field "$BOM" package.maintainer)
    license=$(bom field "$BOM" package.license)
    description=$(bom field "$BOM" package.description)

    if [ ${#TARGETS[@]} -eq 0 ]; then
        while IFS= read -r target_line; do
            [ -n "$target_line" ] && TARGETS+=("$target_line")
        done < <(bom targets "$BOM")
    fi
    [ ${#TARGETS[@]} -gt 0 ] || die "release set has no targets"

    IFS=',' read -r -a requested <<<"$FORMATS"
    for format in "${requested[@]}"; do
        case "$format" in
            deb|rpm|arch|aur|pkg|dmg) ;;
            *) die "unknown format: $format" ;;
        esac
    done

    if [ "$DRY_RUN" = 1 ]; then
        for target in "${TARGETS[@]}"; do
            os=$(target_os "$target") || die "unknown target: $target"
            for format in "${requested[@]}"; do
                format_applies "$format" "$os" || continue
                note "would build $format for $target"
            done
        done
        return 0
    fi

    if printf '%s\n' "${requested[@]}" | grep -qE '^(deb|rpm|arch)$'; then
        command -v "$NFPM" >/dev/null 2>&1 \
            || die "nfpm is required for deb/rpm/arch; install it with: go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest"
    fi
    if printf '%s\n' "${requested[@]}" | grep -qE '^(pkg|dmg)$'; then
        [ "$(uname -s)" = Darwin ] || die "pkg/dmg packaging needs macOS"
        for tool in pkgbuild productbuild hdiutil; do
            command -v "$tool" >/dev/null 2>&1 || die "$tool is required for pkg/dmg"
        done
    fi

    WORK=$(mktemp -d "${TMPDIR:-/tmp}/fungos-package.XXXXXX")
    if [ "$KEEP_WORK" = 1 ]; then
        note "staging under $WORK"
    else
        trap 'rm -rf "$WORK"' EXIT
    fi
    mkdir -p "$OUT"

    linux_targets=()
    for target in "${TARGETS[@]}"; do
        os=$(target_os "$target") || die "unknown target: $target"
        [ "$os" = linux ] && linux_targets+=("$target")
        applicable=0
        for format in "${requested[@]}"; do
            format_applies "$format" "$os" || continue
            applicable=1
        done
        [ "$applicable" = 1 ] || continue
        stage=$(stage_target "$target")
        for format in "${requested[@]}"; do
            format_applies "$format" "$os" || continue
            case "$format" in
                deb) build_nfpm "$target" "$stage" deb >/dev/null ;;
                rpm) build_nfpm "$target" "$stage" rpm >/dev/null ;;
                arch) build_nfpm "$target" "$stage" archlinux >/dev/null ;;
                pkg) pkg=$(build_pkg "$target" "$stage"); note "wrote $pkg" ;;
                dmg)
                    pkg=$OUT/$name-$version-$(target_native_arch "$target").pkg
                    [ -f "$pkg" ] || pkg=$(build_pkg "$target" "$stage")
                    dmg=$(build_dmg "$target" "$pkg")
                    note "wrote $dmg"
                    ;;
            esac
        done
    done

    if printf '%s\n' "${requested[@]}" | grep -qx aur; then
        build_aur "${linux_targets[@]}" >/dev/null
        note "wrote $OUT/aur/$name-bin/PKGBUILD"
    fi

    write_checksums
    say "packages in $OUT"
}

main "$@"
