#!/usr/bin/env bash
#
# bump-version.sh — keep rlshim's version number in sync across the tree.
#
# The build's source of truth is the project() version in CMakeLists.txt; the
# AUR PKGBUILDs pick it up automatically via configure_file(@PROJECT_VERSION@).
# The remaining places carry hand-copied literals that must be kept in lockstep:
#
#   - CMakeLists.txt                                project(rlshim VERSION ...)
#   - src/main.cpp                                  RLSHIM_VERSION fallback #define
#   - packaging/fedora/rlshim.spec                  Version: + %changelog entry
#   - packaging/opensuse/rlshim.spec                Version: + %changelog entry
#   - packaging/flatpak/life.srp.rlshim.metainfo.xml   <release> entry
#
# Usage:
#   scripts/bump-version.sh <version|major|minor|patch> [-m "changelog line"] [--tag]
#
# Examples:
#   scripts/bump-version.sh 1.3.0 -m "Add Wayland support"
#   scripts/bump-version.sh minor -m "Add Wayland support" --tag
#
# With no -m, the changelog files are left untouched (only the Version literals
# move). --tag creates an annotated git tag v<version> after the edits.

set -euo pipefail

# --- locate repo root (this script lives in scripts/) -----------------------
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
ROOT="$(cd -- "${SCRIPT_DIR}/.." &>/dev/null && pwd)"

CMAKE="${ROOT}/CMakeLists.txt"
MAIN="${ROOT}/src/main.cpp"
FEDORA_SPEC="${ROOT}/packaging/fedora/rlshim.spec"
OPENSUSE_SPEC="${ROOT}/packaging/opensuse/rlshim.spec"
FLATPAK_META="${ROOT}/packaging/flatpak/life.srp.rlshim.metainfo.xml"

die() { echo "error: $*" >&2; exit 1; }

# --- parse args -------------------------------------------------------------
BUMP=""
MESSAGE=""
DO_TAG=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        -m|--message) MESSAGE="${2:-}"; shift 2 ;;
        --tag)        DO_TAG=1; shift ;;
        -h|--help)    sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*)           die "unknown option: $1" ;;
        *)            [[ -z "$BUMP" ]] || die "unexpected argument: $1"; BUMP="$1"; shift ;;
    esac
done

[[ -n "$BUMP" ]] || die "missing version argument (a version like 1.3.0, or major|minor|patch)"

# --- read current version from CMakeLists (the source of truth) -------------
CURRENT="$(grep -oE 'project\(rlshim VERSION [0-9]+\.[0-9]+\.[0-9]+' "$CMAKE" \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')" \
    || die "could not read current version from CMakeLists.txt"
[[ -n "$CURRENT" ]] || die "could not read current version from CMakeLists.txt"

IFS='.' read -r CUR_MAJOR CUR_MINOR CUR_PATCH <<< "$CURRENT"

# --- compute the new version ------------------------------------------------
case "$BUMP" in
    major)  NEW="$((CUR_MAJOR + 1)).0.0" ;;
    minor)  NEW="${CUR_MAJOR}.$((CUR_MINOR + 1)).0" ;;
    patch)  NEW="${CUR_MAJOR}.${CUR_MINOR}.$((CUR_PATCH + 1))" ;;
    [0-9]*.[0-9]*.[0-9]*)
        [[ "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "invalid version: $BUMP"
        NEW="$BUMP" ;;
    *) die "argument must be a version like 1.3.0, or one of major|minor|patch" ;;
esac

[[ "$NEW" != "$CURRENT" ]] || die "new version equals current version ($CURRENT)"

echo "Bumping ${CURRENT} -> ${NEW}"

# --- rewrite the plain literals ---------------------------------------------
# CMakeLists: only the project() line, so we don't touch dependency versions.
sed -i -E "s/(project\(rlshim VERSION )${CURRENT//./\\.}/\1${NEW}/" "$CMAKE"
echo "  CMakeLists.txt          project() version"

# main.cpp: the RLSHIM_VERSION fallback #define.
sed -i -E "s/(#define RLSHIM_VERSION \")${CURRENT//./\\.}(\")/\1${NEW}\2/" "$MAIN"
echo "  src/main.cpp            RLSHIM_VERSION fallback"

# rpm specs: the "Version:" field (leading-anchored so changelog lines survive).
sed -i -E "s/^(Version:[[:space:]]+)${CURRENT//./\\.}$/\1${NEW}/" "$FEDORA_SPEC"
sed -i -E "s/^(Version:[[:space:]]+)${CURRENT//./\\.}$/\1${NEW}/" "$OPENSUSE_SPEC"
echo "  fedora/rlshim.spec      Version:"
echo "  opensuse/rlshim.spec    Version:"

# --- verify no stale literals remain in the Version fields ------------------
grep -q "project(rlshim VERSION ${NEW}" "$CMAKE"          || die "CMakeLists.txt version not updated"
grep -q "#define RLSHIM_VERSION \"${NEW}\"" "$MAIN"        || die "main.cpp version not updated"
grep -qE "^Version:[[:space:]]+${NEW}$" "$FEDORA_SPEC"     || die "fedora spec version not updated"
grep -qE "^Version:[[:space:]]+${NEW}$" "$OPENSUSE_SPEC"   || die "opensuse spec version not updated"

# --- changelog entries (only when a message is supplied) --------------------
if [[ -n "$MESSAGE" ]]; then
    RPM_DATE="$(date '+%a %b %d %Y')"     # e.g. Fri Jul 03 2026
    ISO_DATE="$(date '+%Y-%m-%d')"        # e.g. 2026-07-03
    MAINTAINER="Seraphim Pardee <me@srp.life>"

    # rpm %changelog: newest entry goes directly under the "%changelog" line.
    add_rpm_changelog() {
        local spec="$1" tmp
        tmp="$(mktemp)"
        awk -v ver="$NEW" -v date="$RPM_DATE" -v who="$MAINTAINER" -v msg="$MESSAGE" '
            /^%changelog$/ {
                print
                printf "* %s %s - %s-1\n", date, who, ver
                printf "- %s\n", msg
                next
            }
            { print }
        ' "$spec" > "$tmp"
        mv "$tmp" "$spec"
    }
    add_rpm_changelog "$FEDORA_SPEC"
    add_rpm_changelog "$OPENSUSE_SPEC"
    echo "  fedora/rlshim.spec      + %changelog ${NEW}"
    echo "  opensuse/rlshim.spec    + %changelog ${NEW}"

    # flatpak metainfo: newest <release> goes directly under <releases>.
    tmp="$(mktemp)"
    awk -v ver="$NEW" -v date="$ISO_DATE" -v msg="$MESSAGE" '
        /^[[:space:]]*<releases>[[:space:]]*$/ {
            print
            printf "    <release version=\"%s\" date=\"%s\">\n", ver, date
            print  "      <description>"
            printf "        <p>%s</p>\n", msg
            print  "      </description>"
            print  "    </release>"
            next
        }
        { print }
    ' "$FLATPAK_META" > "$tmp"
    mv "$tmp" "$FLATPAK_META"
    echo "  flatpak metainfo        + <release ${NEW}>"
else
    echo "  (no -m given: changelogs left untouched)"
fi

# --- optional git tag -------------------------------------------------------
if [[ "$DO_TAG" -eq 1 ]]; then
    git -C "$ROOT" tag -a "v${NEW}" -m "rlshim v${NEW}"
    echo "  git tag                 v${NEW}"
fi

echo "Done. Review the diff, commit, and (if you didn't pass --tag) tag v${NEW}."
