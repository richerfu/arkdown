use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ResourceIdAllocator {
    assigned_ids: HashMap<(u32, String), u32>,
    resource_ids: HashMap<String, u32>,
    next_id: u32,
}

impl ResourceIdAllocator {
    pub(crate) fn new(module_name: &str, module_names: &[String]) -> Self {
        Self {
            assigned_ids: HashMap::new(),
            resource_ids: HashMap::new(),
            next_id: Self::start_id(module_name, module_names),
        }
    }

    pub(crate) fn assign(&mut self, entry_type: u32, name: &str) -> u32 {
        if let Some(id) = self.assigned_ids.get(&(entry_type, name.to_string())) {
            return *id;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.assigned_ids.insert((entry_type, name.to_string()), id);
        self.resource_ids.entry(name.to_string()).or_insert(id);
        id
    }

    pub(crate) fn resource_ids(&self) -> &HashMap<String, u32> {
        &self.resource_ids
    }

    pub(crate) fn into_resource_ids(self) -> HashMap<String, u32> {
        self.resource_ids
    }

    fn start_id(module_name: &str, module_names: &[String]) -> u32 {
        if module_names.is_empty() {
            return 0x0100_0000;
        }

        let mut names = module_names.to_vec();
        names.sort();
        let index = names
            .iter()
            .position(|name| name == module_name)
            .unwrap_or_default() as u32;
        let mut start_id = (index + 1) * 0x0100_0000;
        if start_id >= 0x0700_0000 {
            start_id += 0x0100_0000;
        }
        start_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_entry_cluster_without_module_names() {
        let mut allocator = ResourceIdAllocator::new("entry", &[]);

        assert_eq!(allocator.assign(0x09, "app_name"), 0x0100_0000);
        assert_eq!(allocator.assign(0x09, "title"), 0x0100_0001);
    }

    #[test]
    fn keeps_same_id_for_same_type_and_name() {
        let mut allocator = ResourceIdAllocator::new("entry", &[]);

        assert_eq!(
            allocator.assign(0x09, "app_name"),
            allocator.assign(0x09, "app_name")
        );
    }

    #[test]
    fn uses_sorted_module_cluster_and_skips_reserved_range() {
        let module_names = vec![
            "m6".to_string(),
            "m5".to_string(),
            "m4".to_string(),
            "m3".to_string(),
            "m2".to_string(),
            "m1".to_string(),
            "m7".to_string(),
        ];
        let mut allocator = ResourceIdAllocator::new("m7", &module_names);

        assert_eq!(allocator.assign(0x09, "app_name"), 0x0800_0000);
    }
}
