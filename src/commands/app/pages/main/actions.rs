//! What the keys do: each asks for what it needs, then runs a command's
//! operation - logging the command line that does the same. One needing the
//! hub fails, saying why, when the hub can't be used.

use anyhow::Result;

use crate::commands::app::App;
use crate::commands::app::components::dialog::{self, Dialog};
use crate::commands::app::components::dir_browser;
use crate::commands::app::window::RunnableWindow;
use crate::commands::{
    Command, get, gitignore, mv, remove, rename_device, rename_folder, share, unshare,
};
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::paths;

impl App {
    /// Asks whether to remove `folder` from this device and the hub, then
    /// removes it.
    pub fn remove(&mut self, folder: FolderId) -> Result<()> {
        let name = self
            .state()?
            .folder(&folder)
            .map(|v| v.folder.name())
            .unwrap_or_default();
        let doing = format!("Removing {name}");
        let question = format!(
            "Are you sure you want to remove {name} from this device and the hub? The files \
             will stay on disk."
        );
        let confirm = dialog::confirm(&doing, &[], &question, "Remove it", "Cancel");
        if !confirm.popup(self)? {
            return Ok(());
        }
        let command = Command::Remove(remove::Args {
            folder: folder.to_string(),
            delete_files: false,
        });
        let doing = format!(
            "Removing {} from this device and the hub",
            self.state()?.label(&folder)
        );
        self.perform(doing, Some(command), move || remove::remove(&folder, false))
    }

    /// Shares `folder` with `device`, or stops sharing it.
    pub fn toggle(&mut self, folder: &FolderId, device: &DeviceId) -> Result<()> {
        // The hub always has the folders shared through it.
        if Some(device) == self.state()?.hub_id() {
            return Ok(());
        }
        if self.state()?.shared(folder, device) {
            self.unshare(folder, device)
        } else {
            self.share(folder, device)
        }
    }

    pub fn share(&mut self, folder: &FolderId, device: &DeviceId) -> Result<()> {
        let name = self.state()?.name(device);
        let command = Command::Share(share::Args {
            folder: folder.to_string(),
            device: name.clone(),
        });
        let doing = format!("Sharing {} with {name}", self.state()?.label(folder));
        let (folder, device) = (folder.clone(), device.clone());
        self.perform(doing, Some(command), move || share::share(&folder, &device))
    }

    pub fn unshare(&mut self, folder: &FolderId, device: &DeviceId) -> Result<()> {
        let name = self.state()?.name(device);
        let command = Command::Unshare(unshare::Args {
            folder: folder.to_string(),
            device: name.clone(),
        });
        let doing = format!("Unsharing {} from {name}", self.state()?.label(folder));
        let (folder, device) = (folder.clone(), device.clone());
        self.perform(doing, Some(command), move || {
            unshare::unshare(&folder, &device)
        })
    }

    /// Gets the hub's share `id` on this device.
    pub fn fetch_folder(&mut self, id: &FolderId) -> Result<()> {
        let Some(folder) = self.state()?.elsewhere(id).cloned() else {
            return Ok(());
        };
        let command = Command::Get(get::Args {
            folder: folder.id.to_string(),
            path: None,
        });
        let doing = format!("Getting {} on this device", folder.name());
        self.perform(doing, Some(command), move || get::get(&folder.id, None))
    }

    pub fn gitignore(&mut self, id: &FolderId) -> Result<()> {
        let Some(folder) = self.state()?.folder(id).map(|view| view.folder.clone()) else {
            return Ok(());
        };
        let path = folder.local_path();
        let command = Command::Gitignore(gitignore::Args {
            path: paths::relative_to_home(&path).into(),
        });
        let doing = format!(
            "Making Syncthing ignore what git ignores in {}",
            folder.name()
        );
        self.perform(doing, Some(command), move || gitignore::gitignore(&path))
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.reload("Refreshing")
    }

    /// Asks for the device's new name, then renames it.
    pub fn rename_device(&mut self, id: &DeviceId) -> Result<()> {
        let old = self.state()?.name(id);
        let doing = format!("Rename {old}");
        let prompt = dialog::prompt(&doing, "Name", &old);
        let Some(name) = prompt.popup(self)? else {
            return Ok(());
        };
        let command = Command::RenameDevice(rename_device::Args {
            device: id.to_string(),
            name: name.clone(),
        });
        let id = id.clone();
        self.perform(
            format!("Renaming {old} to {name}"),
            Some(command),
            move || rename_device::rename(&id, &name),
        )
    }

    /// Asks for the folder's new label, then relabels it.
    pub fn rename_folder(&mut self, id: &FolderId) -> Result<()> {
        let old = self.state()?.label(id);
        let doing = format!("Rename {old}");
        let prompt = dialog::prompt(&doing, "Label", &old);
        let Some(label) = prompt.popup(self)? else {
            return Ok(());
        };
        let command = Command::RenameFolder(rename_folder::Args {
            folder: id.to_string(),
            label: label.clone(),
        });
        let id = id.clone();
        self.perform(
            format!("Renaming {old} to {label}"),
            Some(command),
            move || rename_folder::rename(&id, &label),
        )
    }

    /// Moves a folder's directory to one chosen in the directory browser.
    pub fn move_folder(&mut self, id: &FolderId) -> Result<()> {
        let Some(folder) = self.state()?.folder(id).map(|view| view.folder.clone()) else {
            return Ok(());
        };
        let src = folder.local_path();
        let name = folder.name();
        let title = format!("Move {name} to");
        let marked = self.state()?.synced();
        let browser = dir_browser::choose_target(&title, &src, &marked);
        let Some(dest) = browser.run()? else {
            return Ok(());
        };
        let command = Command::Mv(mv::Args {
            source: paths::relative_to_home(&src),
            dest: paths::relative_to_home(&dest).into(),
            force: false,
        });
        self.perform(
            format!("Moving {name} to {}", paths::relative_to_home(&dest)),
            Some(command),
            move || mv::move_recursive(&folder, &dest, false),
        )
    }
}
