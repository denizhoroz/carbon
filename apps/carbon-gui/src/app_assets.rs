//! App asset source: the kit's default icons plus the Lucide icons Carbon picks.
//! Add an icon by naming it in `icon_assets!` (only listed SVGs are embedded).

use std::borrow::Cow;

use gpui_kit::assets::{Assets, icon_assets};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    ExtraIcons,
    [
        Code,
        GitBranch,
        ListChecks,
        File,
        FileSymlink,
        ChevronsDownUp,
        FilePlus,
        FolderPlus,
        Bot,
        // Command palette rows.
        Save,
        Scissors,
        ClipboardPaste,
        CircleQuestionMark,
        LogOut,
        ListOrdered,
        FileSearch,
        FolderX,
        Move,
        Keyboard,
        // Chat panel.
        Pencil,
        Paperclip,
        TextCursorInput,
        MessageSquare,
        Eraser,
        // Section Bar Manage buttons (Settings is in the kit default set).
        Puzzle,
        Workflow
    ]
);

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
