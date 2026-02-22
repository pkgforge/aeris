use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use super::{
    adapter::{Adapter, AdapterId, AdapterInfo, ProgressSender, Result},
    package::{InstallResult, InstalledPackage, Package, Update},
    privilege::PackageMode,
};

pub struct AdapterManager {
    adapters: HashMap<AdapterId, Arc<dyn Adapter>>,
    disabled: HashSet<String>,
}

impl AdapterManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            disabled: HashSet::new(),
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn Adapter>) {
        let id = adapter.info().id.clone();
        self.adapters.insert(id, adapter);
    }

    pub fn unregister(&mut self, id: &str) {
        self.adapters.remove(id);
        self.disabled.remove(id);
    }

    pub fn set_disabled(&mut self, disabled: HashSet<String>) {
        self.disabled = disabled;
    }

    pub fn set_adapter_enabled(&mut self, id: &str, enabled: bool) {
        if enabled {
            self.disabled.remove(id);
        } else {
            self.disabled.insert(id.to_string());
        }
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        !self.disabled.contains(id)
    }

    pub fn list_adapters(&self) -> Vec<&AdapterInfo> {
        self.adapters.values().map(|a| a.info()).collect()
    }

    pub fn list_adapters_with_status(&self) -> Vec<(AdapterInfo, bool)> {
        self.adapters
            .values()
            .map(|a| {
                let info = a.info().clone();
                let enabled = self.is_enabled(&info.id);
                (info, enabled)
            })
            .collect()
    }

    pub fn any_enabled(&self) -> bool {
        self.adapters.keys().any(|id| self.is_enabled(id))
    }

    pub fn get_adapter(&self, id: &str) -> Option<Arc<dyn Adapter>> {
        self.adapters.get(id).cloned()
    }

    pub async fn search(
        &self,
        query: &str,
        sources: &[String],
        mode: PackageMode,
    ) -> Result<Vec<Package>> {
        let mut results = Vec::new();
        let adapters: Vec<_> = if sources.is_empty() {
            self.adapters
                .values()
                .filter(|a| self.is_enabled(&a.info().id) && a.capabilities().can_search)
                .cloned()
                .collect()
        } else {
            sources
                .iter()
                .filter_map(|id| self.adapters.get(id).cloned())
                .filter(|a| self.is_enabled(&a.info().id) && a.capabilities().can_search)
                .collect()
        };

        for adapter in adapters {
            match adapter.search(query, None, mode).await {
                Ok(pkgs) => results.extend(pkgs),
                Err(e) => log::warn!("Search failed for {}: {e}", adapter.info().id),
            }
        }

        Ok(results)
    }

    pub async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        let mut results = Vec::new();
        for adapter in self
            .adapters
            .values()
            .filter(|a| self.is_enabled(&a.info().id))
        {
            match adapter.list_installed(mode).await {
                Ok(pkgs) => results.extend(pkgs),
                Err(e) => log::warn!("List failed for {}: {e}", adapter.info().id),
            }
        }
        Ok(results)
    }

    pub async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>> {
        let mut results = Vec::new();
        for adapter in self
            .adapters
            .values()
            .filter(|a| self.is_enabled(&a.info().id))
        {
            match adapter.list_updates(mode).await {
                Ok(updates) => results.extend(updates),
                Err(e) => log::warn!("Update check failed for {}: {e}", adapter.info().id),
            }
        }
        Ok(results)
    }

    pub async fn install(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        let mut by_adapter: HashMap<&str, Vec<&Package>> = HashMap::new();
        for pkg in packages {
            by_adapter.entry(&pkg.adapter_id).or_default().push(pkg);
        }

        let mut results = Vec::new();
        for (adapter_id, pkgs) in by_adapter {
            if let Some(adapter) = self.adapters.get(adapter_id) {
                let owned: Vec<Package> = pkgs.into_iter().cloned().collect();
                match adapter.install(&owned, progress.clone(), mode).await {
                    Ok(r) => results.extend(r),
                    Err(e) => log::error!("Install failed for {adapter_id}: {e}"),
                }
            }
        }
        Ok(results)
    }

    pub async fn remove(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<()> {
        let mut by_adapter: HashMap<&str, Vec<&Package>> = HashMap::new();
        for pkg in packages {
            by_adapter.entry(&pkg.adapter_id).or_default().push(pkg);
        }

        for (adapter_id, pkgs) in by_adapter {
            if let Some(adapter) = self.adapters.get(adapter_id) {
                let owned: Vec<Package> = pkgs.into_iter().cloned().collect();
                if let Err(e) = adapter.remove(&owned, progress.clone(), mode).await {
                    log::error!("Remove failed for {adapter_id}: {e}");
                }
            }
        }
        Ok(())
    }

    pub async fn update(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        let mut by_adapter: HashMap<&str, Vec<&Package>> = HashMap::new();
        for pkg in packages {
            by_adapter.entry(&pkg.adapter_id).or_default().push(pkg);
        }

        let mut results = Vec::new();
        for (adapter_id, pkgs) in by_adapter {
            if let Some(adapter) = self.adapters.get(adapter_id) {
                let owned: Vec<Package> = pkgs.into_iter().cloned().collect();
                match adapter.update(&owned, progress.clone(), mode).await {
                    Ok(r) => results.extend(r),
                    Err(e) => log::error!("Update failed for {adapter_id}: {e}"),
                }
            }
        }
        Ok(results)
    }

    pub async fn sync_all(
        &self,
        progress: Option<ProgressSender>,
    ) -> HashMap<AdapterId, Result<()>> {
        let mut results = HashMap::new();
        for (id, adapter) in &self.adapters {
            if self.is_enabled(&adapter.info().id) && adapter.capabilities().can_sync {
                results.insert(id.clone(), adapter.sync(progress.clone()).await);
            }
        }
        results
    }
}
