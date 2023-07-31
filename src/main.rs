// Sparse Array Entity-Component Store:
use anymap::AnyMap;
use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
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

trait PoolRef {
    fn remove(&mut self, entity_id: EntityId);
}

impl<T: Component + Eq> PoolRef for Pool<T> {
    // Remove the component from the given entity
    fn remove(&mut self, entity_id: EntityId) {
        // Remove the index of entity_indices equal to the entity_id
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
    fn reserve_up_to(&mut self, entity_id: EntityId) {
        if entity_id < self.entity_indices.len() {
            return;
        }
        self.entity_indices.resize(entity_id + 1, None);
    }

    // Adds a component, or overrides it if there already is one
    fn add_component(&mut self, entity_id: EntityId, component: T) {
        if entity_id >= self.entity_indices.len() {
            self.reserve_up_to(entity_id);
        }
        if let Some(index) = self.entity_indices[entity_id] {
            // Entity already exists, replace it
            self.entity_list[index] = entity_id;
            self.component_list[index] = component;
        } else {
            self.entity_indices[entity_id] = Some(self.entity_list.len() as usize);
            self.entity_list.push(entity_id);
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

    fn has_component(&self, entity_id: EntityId) -> bool {
        self.entity_indices.get(entity_id).is_none()
    }
}

struct PoolRefStore(Vec<Rc<RefCell<dyn PoolRef>>>);
impl std::fmt::Debug for PoolRefStore {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "PoolRefStore")
    }
}

#[derive(Debug)]
struct EntityStore {
    // Stores Rc<RefCell<Pool<T>>> in an anymap
    // Lets us access the pool of a type, given its type
    store: AnyMap,

    // Stores Rc<RefCell<dyn PoolRef>>> in a vec
    // These are the same pools as in store, but type erased
    // and iterable.
    pool_refs: PoolRefStore,

    // Id of the last entity
    max_entity: EntityId,
}

impl EntityStore {
    fn new() -> Self {
        EntityStore {
            store: AnyMap::new(),
            max_entity: 0,
            pool_refs: PoolRefStore(Vec::new()),
        }
    }

    // Define a new component type for the store
    // Ideally done when there are no entities, or very few
    fn new_component<T: Component + Eq + 'static>(&mut self) {
        let mut pool = Pool::<T>::new();
        pool.reserve_up_to(self.max_entity);

        let pool_rc: Rc<RefCell<Pool<T>>> = Rc::new(RefCell::new(pool));
        self.store.insert(pool_rc.clone());
        self.pool_refs.0.push(pool_rc.clone());
    }

    fn reserve_up_to(&mut self, entity_id: EntityId) {
        if self.max_entity >= entity_id {
            return;
        }
        self.max_entity = entity_id;
    }

    fn get<T: Component + Eq + 'static>(&self) -> Option<&Rc<RefCell<Pool<T>>>> {
        self.store.get::<Rc<RefCell<Pool<T>>>>()
    }

    fn get_mut<T: Component + Eq + 'static>(&mut self) -> Option<&mut Rc<RefCell<Pool<T>>>> {
        self.store.get_mut::<Rc<RefCell<Pool<T>>>>()
    }

    // Add a instance of a component to a entity
    // Note this should not be called when queries are out, only between queries
    // as it performs a borrow_mut on the pool the component is added to
    // THIS IS CALLED COMMAND BUFFERING
    fn add_component<T: Component + Eq + 'static>(&mut self, entity_id: EntityId, component: T) {
        if let Some(pool) = self.store.get_mut::<Rc<RefCell<Pool<T>>>>() {
            let mut pool = pool.borrow_mut();
            pool.add_component(entity_id, component);
        }
    }

    fn remove_component<T: Component + Eq + 'static>(&mut self, entity_id: EntityId) {
        if let Some(pool) = self.store.get_mut::<Rc<RefCell<Pool<T>>>>() {
            let mut pool = pool.borrow_mut();
            pool.remove(entity_id);
        }
    }

    fn entities<T: Component + Eq + 'static>(&self) -> Option<Ref<Vec<EntityId>>> {
        let pool = self.store.get::<Rc<RefCell<Pool<T>>>>()?;
        Some(Ref::map(pool.borrow(), |borrowed| &borrowed.entity_list))
    }

    fn components<T: Component + Eq + 'static>(&self) -> Option<Ref<Vec<T>>> {
        let pool = self.store.get::<Rc<RefCell<Pool<T>>>>()?;
        Some(Ref::map(pool.borrow(), |borrowed| &borrowed.component_list))
    }

    fn components_mut<T: Component + Eq + 'static>(&mut self) -> Option<RefMut<Vec<T>>> {
        let pool = self.store.get::<Rc<RefCell<Pool<T>>>>()?;
        Some(RefMut::map(pool.borrow_mut(), |borrowed| {
            &mut borrowed.component_list
        }))
    }

    fn has_component<T: Component + Eq + 'static>(&self, entity_id: EntityId) -> bool {
        if let Some(pool) = self.store.get::<Rc<RefCell<Pool<T>>>>() {
            pool.borrow().has_component(entity_id)
        } else {
            false
        }
    }

    fn remove_entity(&self, entity_id: EntityId) {
        for pool_ref in &self.pool_refs.0 {
            let mut pool = pool_ref.borrow_mut();
            pool.remove(entity_id);
        }
    }
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

    #[derive(Debug, PartialEq, Eq)]
    struct TestComponent {
        data: i32,
    }

    impl Component for TestComponent {}

    #[test]
    fn entity_store_creation() {
        let store = EntityStore::new();
        assert_eq!(store.max_entity, 0);
        assert_eq!(store.store.len(), 0);
    }

    #[test]
    fn component_registration() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();
        assert_eq!(store.store.len(), 1);
    }

    #[test]
    fn component_addition_removal() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();

        let entity_id = 1;
        store.add_component(entity_id, TestComponent { data: 10 });

        {
            let pool = store.get::<TestComponent>().unwrap();
            let borrowed = pool.borrow();
            assert_eq!(borrowed.get(entity_id).unwrap().data, 10);
        }

        store.remove_component::<TestComponent>(entity_id);

        let pool = store.get::<TestComponent>().unwrap();
        assert!(pool.borrow().get(entity_id).is_none());
    }

    #[test]
    fn entity_removal() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();

        let entity_id = 1;
        store.add_component(entity_id, TestComponent { data: 10 });

        let pool = store.get::<TestComponent>().unwrap();
        let borrowed = pool.borrow();
        assert_eq!(borrowed.get(entity_id).unwrap().data, 10);

        store.remove_entity(entity_id);
        assert!(borrowed.get(entity_id).is_none());
    }

    #[test]
    fn component_iterators() {
        let mut store = EntityStore::new();
        store.new_component::<TestComponent>();

        store.add_component(1, TestComponent { data: 10 });
        store.add_component(2, TestComponent { data: 20 });
        store.add_component(3, TestComponent { data: 30 });

        let pool = store.get::<TestComponent>().unwrap();
        let borrowed = pool.borrow();

        let data: Vec<_> = borrowed.components_iter().map(|(_, c)| c.data).collect();
        assert_eq!(data, vec![10, 20, 30]);

        let mut borrowed_mut = pool.borrow_mut();
        let data: Vec<_> = borrowed_mut
            .components_iter_mut()
            .map(|(_, c)| {
                c.data += 1;
                c.data
            })
            .collect();
        assert_eq!(data, vec![11, 21, 31]);
    }
}
