fn main() -> nih_plug_xtask::Result<()> {
    nih_plug_xtask::chdir_workspace_root()?;
    
    let build_result = nih_plug_xtask::build(&["live_collab_sender".to_owned(), "live_collab_receiver".to_owned()], &["--release".to_owned()]);
    let bundle_sender_result = nih_plug_xtask::bundle(&std::path::Path::new("target"), "live-collab-sender", &["--release".to_owned()], false);
    let bundle_receiver_result = nih_plug_xtask::bundle(&std::path::Path::new("target"), "live-collab-receiver", &["--release".to_owned()], false);

    build_result.and(bundle_sender_result).and(bundle_receiver_result)
}