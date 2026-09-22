use anyhow::{bail, Result};
use nodeclient_types::{safe_join, Instance, Profile, Settings};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        for d in [
            "instances",
            "versions",
            "libraries",
            "assets",
            "java",
            "logs",
        ] {
            fs::create_dir_all(safe_join(&root, d)?)?;
        }
        Ok(Self { root })
    }
    pub fn instance_dir(&self, id: &str) -> Result<PathBuf> {
        nodeclient_types::safe_id(id)?;
        safe_join(&self.root, &format!("instances/{id}"))
    }
    pub fn instances(&self) -> Result<Vec<Instance>> {
        let mut list = vec![];
        for item in fs::read_dir(safe_join(&self.root, "instances")?)? {
            let item = item?;
            if item.file_type()?.is_dir() {
                let id = item.file_name().to_string_lossy().into_owned();
                let path = self.instance_dir(&id)?.join("instance.json");
                if path.exists() {
                    let instance: Instance = serde_json::from_slice(&fs::read(path)?)?;
                    instance.validate()?;
                    if instance.id != id {
                        bail!("Instance directory and metadata ID disagree.");
                    }
                    list.push(instance);
                }
            }
        }
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }
    pub fn get(&self, id: &str) -> Result<Instance> {
        let instance: Instance =
            serde_json::from_slice(&fs::read(self.instance_dir(id)?.join("instance.json"))?)?;
        instance.validate()?;
        if instance.id != id {
            bail!("Instance ID mismatch");
        }
        Ok(instance)
    }
    pub fn save(&self, instance: &Instance) -> Result<()> {
        instance.validate()?;
        let root = self.instance_dir(&instance.id)?;
        fs::create_dir_all(&root)?;
        for d in [
            "mods",
            "config",
            "resourcepacks",
            "shaderpacks",
            "screenshots",
            "logs",
            "saves",
            "crash-reports",
            "backups",
        ] {
            fs::create_dir_all(safe_join(&root, d)?)?;
        }
        write_json(&safe_join(&root, "instance.json")?, instance)
    }
    pub fn delete(&self, id: &str, confirmed: bool) -> Result<()> {
        if !confirmed {
            bail!("Confirm instance deletion first.");
        }
        fs::remove_dir_all(self.instance_dir(id)?)?;
        Ok(())
    }
    pub fn clone_instance(&self, id: &str) -> Result<Instance> {
        let mut instance = self.get(id)?;
        let source = self.instance_dir(id)?;
        instance.id = uuid::Uuid::new_v4().to_string();
        instance.name = format!("{} copy", instance.name);
        instance.last_played = None;
        instance.playtime_seconds = 0;
        let target = self.instance_dir(&instance.id)?;
        copy_tree(&source, &target)?;
        self.save(&instance)?;
        Ok(instance)
    }
    pub fn settings(&self) -> Result<Settings> {
        let p = safe_join(&self.root, "settings.json")?;
        if !p.exists() {
            return Ok(Settings::default());
        }
        let s: Settings = serde_json::from_slice(&fs::read(p)?)?;
        s.validate()?;
        Ok(s)
    }
    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        s.validate()?;
        write_json(&safe_join(&self.root, "settings.json")?, s)
    }
    pub fn accounts(&self) -> Result<Vec<Profile>> {
        let p = safe_join(&self.root, "accounts.json")?;
        if p.exists() {
            Ok(serde_json::from_slice(&fs::read(p)?)?)
        } else {
            Ok(vec![])
        }
    }
    pub fn save_account(&self, p: Profile) -> Result<()> {
        let mut accounts = self.accounts()?;
        accounts.retain(|a| a.id != p.id);
        accounts.push(p);
        write_json(&safe_join(&self.root, "accounts.json")?, &accounts)
    }
    pub fn remove_account(&self, id: &str) -> Result<()> {
        let mut accounts = self.accounts()?;
        accounts.retain(|a| a.id != id);
        write_json(&safe_join(&self.root, "accounts.json")?, &accounts)
    }
}
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let e = entry?;
        let name = e.file_name().to_string_lossy().to_string();
        let from = safe_join(source, &name)?;
        let to = safe_join(target, &name)?;
        if e.file_type()?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(from, to)?;
        }
    }
    Ok(())
}
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Configuration has no parent directory"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(value)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instance_validation() {
        let i:Instance=serde_json::from_value(serde_json::json!({"id":"main","name":"Main","minecraftVersion":"1.21","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":4096}})).unwrap();
        assert!(i.validate().is_ok());
        let mut bad = i;
        bad.memory.minimum_mb = 8192;
        assert!(bad.validate().is_err());
    }
}
#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn atomic_replacement_and_instance_lifecycle() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().to_owned()).unwrap();
        let mut settings = Settings::default();
        store.save_settings(&settings).unwrap();
        settings.theme = "light".into();
        store.save_settings(&settings).unwrap();
        assert_eq!(store.settings().unwrap().theme, "light");
        let instance:Instance=serde_json::from_value(serde_json::json!({"id":"test","name":"Fixture","minecraftVersion":"test","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}})).unwrap();
        store.save(&instance).unwrap();
        std::fs::write(
            store.instance_dir("test").unwrap().join("saves/world.txt"),
            b"world fixture",
        )
        .unwrap();
        let cloned = store.clone_instance("test").unwrap();
        assert_ne!(cloned.id, "test");
        assert!(store
            .instance_dir(&cloned.id)
            .unwrap()
            .join("saves/world.txt")
            .exists());
        assert!(store.delete("test", false).is_err());
        store.delete("test", true).unwrap();
        assert_eq!(store.instances().unwrap().len(), 1);
    }
}
