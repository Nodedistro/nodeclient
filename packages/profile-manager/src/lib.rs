use anyhow::{bail, Result};
use nodeclient_types::{safe_join, Instance, Profile, Settings};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub struct Store {
    pub root: PathBuf,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceInput {
    pub id: Option<String>,
    #[serde(default = "default_engine")]
    pub engine: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: nodeclient_types::Loader,
    pub java: nodeclient_types::Java,
    pub memory: nodeclient_types::Memory,
}
fn default_engine() -> String { "java".into() }
impl Store {
    pub fn prepare_instance(&self, input: InstanceInput) -> Result<Instance> {
        let previous = input.id.as_deref().map(|id| self.get(id)).transpose()?;
        if previous.as_ref().is_some_and(|old| old.engine != input.engine) {
            bail!("Create a separate instance to use another engine. Existing worlds will not be converted automatically.");
        }
        let instance = Instance {
            id: input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: input.name.trim().to_owned(),
            edition: "java".into(),
            engine: input.engine,
            minecraft_version: input.minecraft_version,
            loader: input.loader,
            java: input.java,
            memory: input.memory,
            last_played: previous.as_ref().and_then(|i| i.last_played),
            playtime_seconds: previous.as_ref().map_or(0, |i| i.playtime_seconds),
        };
        instance.validate()?;
        Ok(instance)
    }
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
                    // Preserve unsupported legacy instances on disk without listing them.
                    if instance.edition != "java" {
                        continue;
                    }
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
    pub fn create_horizon_copy(&self, id: &str) -> Result<Instance> {
        let source_instance = self.get(id)?;
        if source_instance.engine == "horizon" { bail!("This instance already uses Horizon."); }
        let instance = Instance {
            id: uuid::Uuid::new_v4().to_string(), name: format!("{} · Rust", source_instance.name),
            edition: "java".into(), engine: "horizon".into(), minecraft_version: "26.3".into(),
            loader: nodeclient_types::Loader { r#type: "vanilla".into(), version: None },
            java: nodeclient_types::Java { mode: "automatic".into(), path: None },
            memory: source_instance.memory.clone(), last_played: None, playtime_seconds: 0,
        };
        instance.validate()?;
        let source_saves = self.instance_dir(id)?.join("saves");
        let target = self.instance_dir(&instance.id)?;
        let result = (|| -> Result<()> {
            self.save(&instance)?;
            if source_saves.is_dir() { copy_tree(&source_saves, &target.join("saves"))?; }
            Ok(())
        })();
        if result.is_err() && target.exists() { let _ = fs::remove_dir_all(&target); }
        result?;
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
    fn existing_engines_are_preserved_and_cannot_be_switched() {
        let root=tempfile::tempdir().unwrap();let store=Store::new(root.path().to_owned()).unwrap();
        let mut value=serde_json::json!({"name":"Native","engine":"horizon","minecraftVersion":"26.3","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}});
        let instance=store.prepare_instance(serde_json::from_value(value.clone()).unwrap()).unwrap();
        store.save(&instance).unwrap();assert_eq!(store.get(&instance.id).unwrap().engine,"horizon");
        value["id"]=instance.id.clone().into();value["engine"]="java".into();
        assert!(store.prepare_instance(serde_json::from_value(value.clone()).unwrap()).is_err());
        value["id"]=serde_json::Value::Null;value["engine"]="horizon".into();value["minecraftVersion"]="1.21".into();
        assert!(store.prepare_instance(serde_json::from_value(value).unwrap()).is_err());
    }
    #[test]
    fn horizon_copy_preserves_worlds_and_leaves_the_java_instance_unchanged() {
        let root=tempfile::tempdir().unwrap();let store=Store::new(root.path().to_owned()).unwrap();
        let input=serde_json::json!({"name":"Java world","engine":"java","minecraftVersion":"1.21","loader":{"type":"fabric","version":"0.16.0"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}});
        let java=store.prepare_instance(serde_json::from_value(input).unwrap()).unwrap();store.save(&java).unwrap();
        std::fs::write(store.instance_dir(&java.id).unwrap().join("saves/level.dat"),b"world").unwrap();
        std::fs::write(store.instance_dir(&java.id).unwrap().join("mods/java-only.jar"),b"mod").unwrap();
        let native=store.create_horizon_copy(&java.id).unwrap();
        assert_eq!(native.engine,"horizon");assert_eq!(native.minecraft_version,"26.3");assert_eq!(native.loader.r#type,"vanilla");
        assert_eq!(std::fs::read(store.instance_dir(&native.id).unwrap().join("saves/level.dat")).unwrap(),b"world");
        assert!(!store.instance_dir(&native.id).unwrap().join("mods/java-only.jar").exists());
        assert_eq!(store.get(&java.id).unwrap().engine,"java");
    }
    #[test]
    fn rust_creates_ids_and_preserves_history_when_editing() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().to_owned()).unwrap();
        let input = serde_json::json!({"name":"  Adventure  ","minecraftVersion":"1.21","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}});
        let mut instance = store.prepare_instance(serde_json::from_value(input.clone()).unwrap()).unwrap();
        assert!(uuid::Uuid::parse_str(&instance.id).is_ok());
        assert_eq!(instance.name, "Adventure");
        assert_eq!(instance.edition, "java");
        instance.last_played = Some(123);
        instance.playtime_seconds = 456;
        store.save(&instance).unwrap();
        let mut edit = input.clone();
        edit["id"] = instance.id.clone().into();
        edit["name"] = "Renamed".into();
        let updated = store.prepare_instance(serde_json::from_value(edit.clone()).unwrap()).unwrap();
        assert_eq!(updated.id, instance.id);
        assert_eq!(updated.last_played, Some(123));
        assert_eq!(updated.playtime_seconds, 456);
        edit["id"] = "missing".into();
        assert!(store.prepare_instance(serde_json::from_value(edit).unwrap()).is_err());
        let mut forged = input;
        forged["playtimeSeconds"] = 999.into();
        assert!(serde_json::from_value::<InstanceInput>(forged).is_err());
    }
    #[test]
    fn unsupported_instances_are_preserved_but_not_listed() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().to_owned()).unwrap();
        let mut instance: Instance = serde_json::from_value(serde_json::json!({"id":"legacy","name":"Legacy","edition":"bedrock","minecraftVersion":"bedrock","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}})).unwrap();
        assert!(store.save(&instance).is_err());
        let directory = store.instance_dir("legacy").unwrap();
        fs::create_dir_all(&directory).unwrap();
        let metadata = directory.join("instance.json");
        let original = serde_json::to_vec(&instance).unwrap();
        fs::write(&metadata, &original).unwrap();
        instance.id = "java".into();
        instance.edition = "java".into();
        instance.minecraft_version = "1.21".into();
        store.save(&instance).unwrap();
        let visible = store.instances().unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "java");
        assert!(store.get("legacy").is_err());
        assert_eq!(fs::read(metadata).unwrap(), original);
    }
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
