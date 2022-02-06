"""# Rules

- [crates_repository](#crates_repository)
- [crate.spec](#cratespec)
- [crate.workspace_member](#crateworkspace_member)
- [crate.annotation](#crateannotation)
- [render_config](#render_config)
- [splicing_config](#splicing_config)

"""

load(
    "//private:crates_repository.bzl",
    _crate = "crate",
    _crates_repository = "crates_repository",
    _render_config = "render_config",
    _splicing_config = "splicing_config",
)

crates_repository = _crates_repository
crate = _crate
render_config = _render_config
splicing_config = _splicing_config
