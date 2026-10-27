#[derive(Debug, Clone)]
pub(crate) enum ShellCommand {
    LoadAuthenticatedShell,
    /// Reads the settings again, nothing more: the shell's read-only refresh (C4b item 2).
    LoadSettings,
    ResolveStartupRoute,
    ManualRefresh,
}
