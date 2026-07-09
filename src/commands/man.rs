use std::path::Path;

use anyhow::Result;

/// Print the top-level man page to stdout, or with `dir` write one page per
/// (sub)command into it: clockify.1, clockify-start.1, clockify-expenses-add.1, ...
pub fn run(cmd: clap::Command, dir: Option<&Path>) -> Result<()> {
    let Some(dir) = dir else {
        clap_mangen::Man::new(cmd).render(&mut std::io::stdout())?;
        return Ok(());
    };
    std::fs::create_dir_all(dir)?;
    let version = cmd.get_version().map(str::to_owned);
    render(cmd, "clockify", version.as_deref(), dir)
}

fn render(cmd: clap::Command, name: &str, version: Option<&str>, dir: &Path) -> Result<()> {
    let mut page = cmd.clone().name(name.to_owned());
    // Subcommands don't carry the version; propagate it so every page's footer has it.
    if let Some(version) = version {
        page = page.version(version.to_owned());
    }
    let mut buf = Vec::new();
    clap_mangen::Man::new(page).render(&mut buf)?;
    std::fs::write(dir.join(format!("{name}.1")), buf)?;
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }
        let sub_name = format!("{name}-{}", sub.get_name());
        render(sub.clone(), &sub_name, version, dir)?;
    }
    Ok(())
}
