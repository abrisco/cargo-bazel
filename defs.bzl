"""# Rules

- [crates_repository](#crates_repository)
- [crate.spec](#cratespec)
- [crate.workspace_member](#crateworkspace_member)
- [crate.annotation](#crateannotation)
- [render_config](#render_config)
- [splicing_config](#splicing_config)

"""

load(
    "//private:crate.bzl",
    _crate = "crate",
)
load(
    "//private:crates_repository.bzl",
    _crates_repository = "crates_repository",
)
load(
    "//private:generate_utils.bzl",
    _render_config = "render_config",
)
load(
    "//private:splicing_utils.bzl",
    _splicing_config = "splicing_config",
)

crate = _crate
crates_repository = _crates_repository
render_config = _render_config
splicing_config = _splicing_config
