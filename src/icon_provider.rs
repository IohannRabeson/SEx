use std::{
    cell::RefCell,
    collections::{
        btree_map::Entry::{Occupied, Vacant},
        BTreeMap,
    },
    ffi::OsString,
    path::Path,
};

use file_icon_provider::{get_file_icon, Error, Icon};
use iced::widget::image;

use crate::ui;

pub struct IconProvider {
    cache: RefCell<BTreeMap<(u16, OsString), image::Handle>>,
    size: u16,
}

impl Default for IconProvider {
    fn default() -> Self {
        Self {
            cache: RefCell::new(BTreeMap::new()),
            #[cfg(target_os = "macos")]
            size: (ui::ICON_SIZE * 2) as u16,
            #[cfg(not(target_os = "macos"))]
            size: ui::ICON_SIZE as u16,
        }
    }
}
impl IconProvider {
    /// Retrieves the icon for a given file.
    pub fn icon(&self, path: impl AsRef<Path>) -> Result<image::Handle, Error> {
        let path = path.as_ref();
        let get_icon = |path| get_file_icon(path, self.size).map(Self::convert);

        match path.extension() {
            Some(extension) => match self
                .cache
                .borrow_mut()
                .entry((self.size, extension.to_owned()))
            {
                Vacant(vacant_entry) => Ok(vacant_entry.insert(get_icon(path)?).clone()),
                Occupied(occupied_entry) => Ok(occupied_entry.get().clone()),
            },
            // No extension then no caching.
            None => get_icon(path),
        }
    }

    fn convert(icon: Icon) -> image::Handle {
        image::Handle::from_rgba(icon.width, icon.height, icon.pixels)
    }
}
