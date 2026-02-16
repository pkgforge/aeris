use std::{collections::HashMap, sync::Arc};

use super::{
    adapter::{Adapter, AdapterError, AdapterId, AdapterInfo, ProgressSender, Result},
    package::{InstallResult, InstalledPackage, Package, Update},
    privilege::PackageMode,
};

pub struct AdapterManager {
    adapters: HashMap<AdapterId, Arc<dyn Adapter>>,
}

impl AdapterManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn Adapter>) {
        let id = adapter.info().id.clone();
        self.adapters.insert(id, adapter);
    }

    pub fn list_adapters(&self) -> Vec<&AdapterInfo> {
        self.adapters.values().map(|a| a.info()).collect()
    }

    pub fn get_adapter(&self, id: &str) -> Option<Arc<dyn Adapter>> {
        self.adapters.get(id).cloned()
    }

    pub async fn search(&self, query: &str, sources: &[String]) -> Result<Vec<Package>> {
        let mut results = Vec::new();
        let adapters: Vec<_> = if sources.is_empty() {
            self.adapters
                .values()
                .filter(|a| a.info().enabled && a.capabilities().can_search)
                .cloned()
                .collect()
        } else {
            sources
                .iter()
                .filter_map(|id| self.adapters.get(id).cloned())
                .filter(|a| a.info().enabled && a.capabilities().can_search)
                .collect()
        };

        for adapter in adapters {
            match adapter.search(query, None).await {
                Ok(pkgs) => results.extend(pkgs),
                Err(e) => log::warn!("Search failed for {}: {e}", adapter.info().id),
            }
        }

        Ok(results)
    }

    pub async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        let mut results = Vec::new();
        for adapter in self.adapters.values().filter(|a| a.info().enabled) {
            match adapter.list_installed(mode).await {
                Ok(pkgs) => results.extend(pkgs),
                Err(e) => log::warn!("List failed for {}: {e}", adapter.info().id),
            }
        }
        Ok(results)
    }

    pub async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>> {
        let mut results = Vec::new();
        for adapter in self.adapters.values().filter(|a| a.info().enabled) {
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
    ) -> Result<Vec<InstallResult>> {
        let mut by_adapter: HashMap<&str, Vec<&Package>> = HashMap::new();
        for pkg in packages {
            by_adapter.entry(&pkg.adapter_id).or_default().push(pkg);
        }

        let mut results = Vec::new();
        for (adapter_id, pkgs) in by_adapter {
            if let Some(adapter) = self.adapters.get(adapter_id) {
                let owned: Vec<Package> = pkgs.into_iter().cloned().collect();
                match adapter.install(&owned, progress.clone()).await {
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
            if adapter.info().enabled && adapter.capabilities().can_sync {
                results.insert(id.clone(), adapter.sync(progress.clone()).await);
            }
        }
        results
    }
}
