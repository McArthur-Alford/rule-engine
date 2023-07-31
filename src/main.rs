// Sparse Array Entity-Component Store:
use anymap::AnyMap;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type EntityId = usize;

trait Component {}

// https://gist.github.com/dakom/82551fff5d2b843cbe1601bbaff2acbf
// http://reports-archive.adm.cs.cmu.edu/anon/1995/CMU-CS-95-113.pdf

#[derive(Debug, PartialEq, Eq)]
struct Pool<T: Component + Eq> {
    // A sparse array, values are integers which index EntityList
    // Index of elements is their EntityId
    entity_indices: Vec<Option<EntityId>>,

    // A packed array, contains integers which are EntityIds
    // Index is meaningless other than that it is correct from entity_indices
    entity_list: Vec<EntityId>,

    // A packed array, contains the components
    component_list: Vec<T>,
}

trait PoolRemoval {
    fn remove(&mut self, entity_id: EntityId);
}

impl<T: Component + Eq> PoolRemoval for Pool<T> {
    // Remove the component from the given entity
    fn remove(&mut self, entity_id: EntityId) {
        // Remove the index of entity_indices equal to the entityId
        if let Some(entity_index) = self.entity_indices.get_mut(entity_id) {
            if let Some(entity_index_copy) = *entity_index {
                *entity_index = None;

                // First of all, remove the entity_list and component_list using a swap_pop
                let new_entity_id = self.entity_list.swap_remove(entity_index_copy);
                self.component_list.swap_remove(entity_index_copy);

                // Update the entity_indices value that previously pointed to the end
                if let Some(old_entity_id) = self.entity_indices.get_mut(new_entity_id) {
                    *old_entity_id = Some(new_entity_id);
                }
            }
        }
    }
}

impl<T: Component + Eq> Pool<T> {
    fn new() -> Self {
        Pool {
            entity_indices: Vec::new(),
            entity_list: Vec::new(),
            component_list: Vec::new(),
        }
    }

    fn new_entity(&mut self) -> EntityId {
        self.entity_indices.push(None);
        return self.entity_indices.len() - 1;
    }

    // Ensures that the entity list is allocated up to (and including) a given entity id
    fn reserve_up_to(&mut self, entityId: EntityId) {
        if entityId < self.entity_indices.len() {
            return;
        }
        self.entity_indices.resize(entityId + 1, None);
    }

    // Adds a component, or overrides it if there already is one
    fn add_component(&mut self, entityId: EntityId, component: T) {
        if entityId >= self.entity_indices.len() {
            self.reserve_up_to(entityId);
        }
        if let Some(index) = self.entity_indices[entityId] {
            // Entity already exists, replace it
            self.entity_list[index] = entityId;
            self.component_list[index] = component;
        } else {
            self.entity_indices[entityId] = Some(self.entity_list.len() as usize);
            self.entity_list.push(entityId);
            self.component_list.push(component);
        }
    }

    // Returns the length of entity_list/component_list (they should be the same)
    fn len(&mut self) -> usize {
        self.entity_list.len()
    }

    fn entities(&self) -> Vec<&EntityId> {
        self.entity_list.iter().collect()
    }

    fn components(&self) -> Vec<(&EntityId, &T)> {
        self.entity_list
            .iter()
            .zip(self.component_list.iter())
            .collect()
    }

    fn get(&self, entity_id: EntityId) -> Option<&T> {
        Some(&self.component_list[(*self.entity_indices.get(entity_id)?)?])
    }

    fn get_mut(&mut self, entity_id: EntityId) -> Option<&T> {
        Some(&mut self.component_list[(*self.entity_indices.get(entity_id)?)?])
    }

    fn components_mut(&mut self) -> Vec<(&EntityId, &mut T)> {
        self.entity_list
            .iter()
            .zip(self.component_list.iter_mut())
            .collect()
    }

    fn components_iter(&self) -> impl Iterator<Item = (&EntityId, &T)> {
        self.entity_list.iter().zip(self.component_list.iter())
    }

    fn components_iter_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut T)> {
        self.entity_list.iter().zip(self.component_list.iter_mut())
    }

    fn has_component(&self, entityId: EntityId) -> bool {
        self.entity_indices.get(entityId).is_none()
    }
}

struct PoolRemovalStore(Vec<Rc<RefCell<Box<dyn PoolRemoval>>>>);
impl std::fmt::Debug for PoolRemovalStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "PoolRemovalStore")
    }
}

#[derive(Debug)]
struct EntityStore {
    store: AnyMap,

    // Id of the last entity
    max_entity: EntityId,

    // Component pools for removal
    pool_refs: PoolRemovalStore,
}

impl EntityStore {
    fn new() -> Self {
        EntityStore {
            store: AnyMap::new(),
            max_entity: 0,
            pool_refs: PoolRemovalStore(Vec::new()),
        }
    }

    // Define a new component type for the store
    // Ideally done when there are no entities, or very few
    fn new_component<T: Component + Eq + 'static>(&mut self) {
        let mut pool = Pool::<T>::new();
        pool.reserve_up_to(self.max_entity);
        self.store.insert(pool);
    }

    fn reserve_up_to(&mut self, entityId: EntityId) {
        if self.max_entity >= entityId {
            return;
        }
        self.max_entity = entityId;
    }

    fn get<T: Component + Eq + 'static>(&self) -> Option<&Pool<T>> {
        self.store.get::<Pool<T>>()
    }

    fn get_mut<T: Component + Eq + 'static>(&mut self) -> Option<&mut Pool<T>> {
        self.store.get_mut::<Pool<T>>()
    }

    // Add a instance of a component to a entity
    fn add_component<T: Component + Eq + 'static>(&mut self, entityId: EntityId, component: T) {
        if let Some(pool) = self.store.get_mut::<Pool<T>>() {
            pool.add_component(entityId, component);
        }
    }

    fn remove_component<T: Component + Eq + 'static>(&mut self, entityId: EntityId) {
        if let Some(pool) = self.store.get_mut::<Pool<T>>() {
            pool.remove(entityId);
        }
    }

    fn entities<T: Component + Eq + 'static>(&self) -> Option<Vec<&EntityId>> {
        if let Some(pool) = self.store.get::<Pool<T>>() {
            Some(pool.entity_list.iter().collect())
        } else {
            None
        }
    }

    fn components<T: Component + Eq + 'static>(&self) -> Option<Vec<(&EntityId, &T)>> {
        if let Some(pool) = self.store.get::<Pool<T>>() {
            Some(pool.components())
        } else {
            None
        }
    }

    fn components_iter<T: Component + Eq + 'static>(
        &self,
    ) -> impl Iterator<Item = (&EntityId, &T)> {
        self.store
            .get::<Pool<T>>()
            .into_iter()
            .flat_map(|pool| pool.components_iter())
    }

    fn components_mut<T: Component + Eq + 'static>(&mut self) -> Option<Vec<(&EntityId, &mut T)>> {
        if let Some(pool) = self.store.get_mut::<Pool<T>>() {
            Some(pool.components_mut())
        } else {
            None
        }
    }

    fn components_iter_mut<T: Component + Eq + 'static>(
        &mut self,
    ) -> impl Iterator<Item = (&EntityId, &mut T)> {
        self.store
            .get_mut::<Pool<T>>()
            .into_iter()
            .flat_map(|pool| pool.components_iter_mut())
    }

    fn has_component<T: Component + Eq + 'static>(&self, entity_id: EntityId) -> bool {
        if let Some(pool) = self.store.get::<Pool<T>>() {
            pool.has_component(entity_id)
        } else {
            false
        }
    }

    fn remove_entity(&self, entity_id: EntityId) {}
}

trait View {}

// Extractor Pattern, semi-simply explained
// https://blog.logrocket.com/rust-bevy-entity-component-system/

// Spatial stuff using logic programming:
// https://cgi.cse.unsw.edu.au/~eptcs/paper.cgi?ICLP2021.34.pdf
fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct TestComponent {
        value: i32,
    }

    impl Component for TestComponent {}

    #[test]
    fn new_pool() {
        let pool: Pool<TestComponent> = Pool::new();
        assert_eq!(pool.entity_indices.len(), 0);
        assert_eq!(pool.entity_list.len(), 0);
        assert_eq!(pool.component_list.len(), 0);
    }

    #[test]
    fn new_entity() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let id = pool.new_entity();
        assert_eq!(id, 0);
    }

    #[test]
    fn add_component() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let id = pool.new_entity();
        let component = TestComponent { value: 42 };
        pool.add_component(id, component.clone());
        assert_eq!(pool.entity_list.len(), 1);
        assert_eq!(pool.component_list.len(), 1);
        assert_eq!(pool.component_list[0], component);
    }

    #[test]
    fn remove_component() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let id = pool.new_entity();
        let component = TestComponent { value: 42 };
        pool.add_component(id, component.clone());
        assert_eq!(pool.entity_list.len(), 1);
        assert_eq!(pool.component_list.len(), 1);
        pool.remove(id);
        assert_eq!(pool.entity_list.len(), 0);
        assert_eq!(pool.component_list.len(), 0);
    }

    #[test]
    fn new_component_in_store() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();
        assert!(store.get::<TestComponent>().is_some());
    }

    #[test]
    fn add_component_in_store() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();
        let component = TestComponent { value: 42 };
        store.add_component(0, component.clone());
        let pool = store.get::<TestComponent>().unwrap();
        assert_eq!(pool.component_list.len(), 1);
        assert_eq!(pool.component_list[0], component);
    }

    #[test]
    fn reserve_up_to() {
        let mut pool: Pool<TestComponent> = Pool::new();
        pool.reserve_up_to(5);
        assert_eq!(pool.entity_indices.len(), 6);
        assert!(pool.entity_indices.iter().all(|&x| x == None));
    }

    #[test]
    fn add_component_non_existent_entity() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let component = TestComponent { value: 42 };
        pool.add_component(3, component.clone());
        assert_eq!(pool.entity_list.len(), 1);
        assert_eq!(pool.component_list.len(), 1);
        assert_eq!(pool.component_list[0], component);
        assert_eq!(pool.entity_indices.len(), 4);
    }

    #[test]
    fn remove_non_existent_entity() {
        let mut pool: Pool<TestComponent> = Pool::new();
        pool.remove(3);
        assert_eq!(pool.entity_list.len(), 0);
        assert_eq!(pool.component_list.len(), 0);
        assert_eq!(pool.entity_indices.len(), 0);
    }

    #[test]
    fn add_multiple_components_to_same_entity() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let id = pool.new_entity();
        let component1 = TestComponent { value: 42 };
        let component2 = TestComponent { value: 100 };
        pool.add_component(id, component1.clone());
        pool.add_component(id, component2.clone());
        assert_eq!(pool.entity_list.len(), 1);
        assert_eq!(pool.component_list.len(), 1);
        assert_eq!(pool.component_list[0], component2);
    }

    #[test]
    fn remove_entity_with_no_component() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let id = pool.new_entity();
        pool.remove(id);
        assert_eq!(pool.entity_list.len(), 0);
        assert_eq!(pool.component_list.len(), 0);
        assert_eq!(pool.entity_indices[id], None);
    }

    #[test]
    fn add_components_in_non_sequential_order() {
        let mut pool: Pool<TestComponent> = Pool::new();
        let component = TestComponent { value: 42 };
        pool.add_component(3, component.clone());
        assert_eq!(pool.entity_list.len(), 1);
        assert_eq!(pool.component_list.len(), 1);
        assert_eq!(pool.component_list[0], component);
        assert_eq!(pool.entity_indices[3].unwrap(), 0);
    }

    #[test]
    fn reserve_up_to_in_store_with_no_components() {
        let mut store = EntityStore::new();
        store.reserve_up_to(5);
        assert_eq!(store.max_entity, 5);
    }

    #[test]
    fn add_component_in_store_with_no_component_type() {
        let mut store = EntityStore::new();
        let component = TestComponent { value: 42 };
        store.add_component(0, component.clone());
        assert!(store.get::<TestComponent>().is_none());
    }

    #[test]
    fn get_non_existent_component_in_store() {
        let store = EntityStore::new();
        let result = store.get::<TestComponent>();
        assert!(result.is_none());
    }

    #[test]
    fn remove_component_from_non_existent_entity_in_store() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();
        let id = 3;
        if let Some(pool) = store.get_mut::<TestComponent>() {
            pool.remove(id);
        }
        let pool = store.get::<TestComponent>().unwrap();
        assert_eq!(pool.entity_list.len(), 0);
        assert_eq!(pool.component_list.len(), 0);
        assert!(pool.entity_indices.get(id).is_none());
    }
}
