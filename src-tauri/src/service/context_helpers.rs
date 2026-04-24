use super::*;

pub(super) fn manifests_match(
    left: &ProjectContextManifest,
    right: &ProjectContextManifest,
) -> bool {
    normalize_manifest_value(left.project.as_deref())
        == normalize_manifest_value(right.project.as_deref())
        && normalize_manifest_value(left.instructions.as_deref())
            == normalize_manifest_value(right.instructions.as_deref())
        && normalize_manifest_value(left.memory_dir.as_deref())
            == normalize_manifest_value(right.memory_dir.as_deref())
}

pub(super) fn normalize_manifest_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
