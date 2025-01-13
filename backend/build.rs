fn main() {
    build_data::set_GIT_BRANCH();
    build_data::set_GIT_COMMIT();
    build_data::set_GIT_DIRTY();
    // build_data::set_SOURCE_TIMESTAMP();
    build_data::no_debug_rebuilds();

    let git_commit = build_data::get_git_commit().unwrap();
    let git_dirty = build_data::get_git_dirty().unwrap();

    let value = format!("{}{}", git_commit, if git_dirty { " dirty" } else { "" });
    println!("cargo:rustc-env=VERSION_INFO={value}");
}
