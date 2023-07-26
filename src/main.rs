// Sparse Array Entity-Component Store:
use anymap::AnyMap;

type EntityId = usize;

trait Component {}

impl Component for i64 {}

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

    // A packed array, contains component data of pool type
    // Is aligned with entity_list s.t. entity_list[N] has component data of
    // component_list[N]
    component_list: Vec<T>,
}

impl<T: Component + Eq> Pool<T> {
    fn new() -> Self {
        Pool {
            entity_indices: Vec::new(),
            entity_list: Vec::new(),
            component_list: Vec::new(),
        }
    }

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

    //
    fn add_component(&mut self, entityId: EntityId, component: T) {
        if entityId >= self.entity_indices.len() {
            self.reserve_up_to(entityId);
        }
        self.entity_indices[entityId] = Some(self.entity_list.len() as usize);
        self.entity_list.push(entityId);
        self.component_list.push(component);
    }

    // Returns the length of entity_list/component_list (they should be the same)
    fn len(&mut self) -> usize {
        self.entity_list.len()
    }
}

struct EntityStore {
    map: AnyMap,
}

// Spatial stuff using logic programming:
// https://cgi.cse.unsw.edu.au/~eptcs/paper.cgi?ICLP2021.34.pdf

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use crate::Pool;

    #[test]
    fn test1() {
        let mut pool = Pool::<i64>::new();
        pool.add_component(1 as usize, 64);
        assert_eq!(
            pool,
            Pool {
                entity_indices: vec![None, Some(0 as usize)],
                entity_list: vec![1],
                component_list: vec![64]
            }
        );
    }
}
