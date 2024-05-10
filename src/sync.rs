use std::cell::RefCell;
use std::cmp::{min, Ordering};
use std::fmt::{Display, Formatter};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use xxhash_rust::xxh3::Xxh3;

enum Visit {
    /// Continue visiting
    Continue,

    /// Skip visiting children of current node
    Skip,

    /// Stop all further visits and return immediately
    Stop,
}

struct TreeSet<T>
where
    T: AsRef<[u8]> + Default + Clone + Eq,
{
    root: Node<T>,
}

#[inline]
fn prefix_match<T>(prefix: &[T], path: &Vec<T>, node: &Node<T>) -> bool
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Serialize + Ord,
{
    if prefix.is_empty() || path.is_empty() {
        true
    } else {
        let mut full_path = path.clone();
        full_path.push(node.value.clone());
        let check_prefix_len = min(full_path.len(), prefix.len());

        full_path[0..check_prefix_len] == prefix[0..check_prefix_len]
    }
}

impl<T> TreeSet<T>
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Serialize + Ord,
{
    pub fn new() -> Self {
        Self {
            root: Node::default(),
        }
    }

    pub fn rehash(&mut self) {
        self.root.rehash();
    }

    pub fn insert(&mut self, path: &[T], value: T) {
        self.root.insert(path, value, None, false);
    }

    pub fn insert_update(&mut self, path: &[T], value: T) {
        self.root.insert(path, value, None, true);
    }

    pub fn insert_with_hash_update(&mut self, path: &[T], value: T, hash: u64) {
        self.root.insert(path, value, Some(hash), true);
    }

    pub fn insert_with_hash(&mut self, path: &[T], value: T, hash: u64) {
        self.root.insert(path, value, Some(hash), false);
    }

    pub fn root(&self) -> &Node<T> {
        &self.root
    }

    /// Visit the tree in pre-order
    pub fn visit(&self, fun: &dyn Fn(&Vec<T>, &Node<T>) -> Visit) {
        let mut path: Vec<T> = Vec::new();
        self.root.visit_children(&mut path, fun);
    }

    pub fn leafs(&self, prefix: &[T]) -> Vec<T> {
        let leafs = RefCell::new(Vec::new());

        self.visit(&|path, node| -> Visit {
            if !prefix_match(prefix, path, node) {
                return Visit::Skip;
            }

            if node.children.is_empty() {
                leafs.borrow_mut().push(node.value.clone());
            }

            Visit::Continue
        });

        leafs.into_inner()
    }

    pub fn extract_root(&self, depth: usize) -> TreeSet<T> {
        self.extract(&[], depth)
    }

    pub fn extract(&self, prefix: &[T], depth: usize) -> TreeSet<T> {
        let sparse = RefCell::new(TreeSet::new());
        let max_len = prefix.len() + depth;

        self.visit(&|path, node| -> Visit {
            if !prefix_match(prefix, path, node) || path.len() + 1 > max_len {
                return Visit::Skip;
            }

            let mut tree = sparse.borrow_mut();
            tree.insert_with_hash(path, node.value.clone(), node.hash);

            Visit::Continue
        });

        sparse.into_inner()
    }

    fn diff(&self, other: &TreeSet<T>) -> Vec<Diff<T>>
    where
        T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Ord + Serialize,
    {
        let mut path: Vec<T> = Vec::new();

        let self_root = &self.root;
        let other_root = &other.root;

        return self_root.diff(&mut path, other_root);
    }
}

#[derive(Clone)]
struct Node<T>
where
    T: AsRef<[u8]> + Default + Clone + Eq,
{
    value: T,
    level: u64,
    hash: u64,
    manual_hash: Option<u64>,
    children: Vec<Node<T>>,
    dirty: bool,
}

impl<T> Default for Node<T>
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default,
{
    fn default() -> Self {
        Self {
            value: Default::default(),
            level: 0,
            hash: 0,
            children: Vec::new(),
            dirty: true,
            manual_hash: None,
        }
    }
}

impl<T> Serialize for Node<T>
where
    T: AsRef<[u8]> + Default + Clone + Eq + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Node", 3)?;
        state.serialize_field("value", &self.value)?;
        state.serialize_field("hash", &self.hash)?;
        state.serialize_field("children", &self.children)?;
        state.end()
    }
}

#[derive(Default)]
struct U64Buffer([u8; 4]);

impl U64Buffer {
    fn update(&mut self, value: u64) {
        self.0[0] = ((value >> 24) & 0xff) as u8;
        self.0[1] = ((value >> 16) & 0xff) as u8;
        self.0[2] = ((value >> 8) & 0xff) as u8;
        self.0[3] = (value & 0xff) as u8;
    }
}

impl<T> Node<T>
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Ord,
{
    pub fn children(&self) -> &Vec<Node<T>> {
        &self.children
    }

    pub fn visit_children(
        &self,
        path: &mut Vec<T>,
        fun: &dyn Fn(&Vec<T>, &Node<T>) -> Visit,
    ) -> Visit {
        for child in &self.children {
            let result = child.visit(path, fun);

            match result {
                // break out if we should stop
                Visit::Stop => {
                    return Visit::Stop;
                }

                // otherwise continue with the next child
                _ => {}
            }
        }

        Visit::Continue
    }

    pub fn visit(&self, path: &mut Vec<T>, fun: &dyn Fn(&Vec<T>, &Node<T>) -> Visit) -> Visit {
        match fun(path, self) {
            Visit::Continue => {}

            // at this point stop and skip are the same
            // (i.e. we don't want to visit the children)
            other => return other,
        }

        path.push(self.value.clone());
        let result = self.visit_children(path, fun);
        path.pop();

        result
    }

    pub fn update_hash(&mut self) {
        if let Some(hash) = self.manual_hash {
            self.hash = hash;
            self.dirty = false;
            return;
        }

        if !self.dirty {
            return;
        }

        let mut hasher = Xxh3::new();
        let mut buf: U64Buffer = Default::default();

        // hash children
        for child in self.children.iter() {
            buf.update(child.hash);
            hasher.update(&buf.0);
        }

        // hash self
        hasher.update(self.value.as_ref());

        self.hash = hasher.digest();
        self.dirty = false;
    }

    pub fn rehash(&mut self) {
        for child in self.children.iter_mut() {
            child.rehash();
        }

        self.update_hash();
    }

    #[inline]
    fn get_or_create_child(&mut self, value: T, update: bool) -> usize {
        match self
            .children
            // find child with value=value ...
            .binary_search_by(|child| child.value.cmp(&value))
        {
            Ok(position) => position,
            Err(position) => {
                // ... or create a new one at the right position
                let mut node = Node::default();
                node.value = value;
                node.level = self.level + 1;

                if update {
                    node.update_hash();
                }

                self.children.insert(position, node);
                position
            }
        }
    }

    pub fn insert(&mut self, path: &[T], value: T, hash: Option<u64>, update: bool) {
        self.dirty = true;

        match path {
            [] => {
                let position = self.get_or_create_child(value.clone(), update);
                self.children[position].value = value;

                if let Some(hash) = hash {
                    self.children[position].hash = hash;
                    self.children[position].manual_hash = Some(hash);
                } else if update {
                    self.children[position].update_hash();
                }
            }
            [head, tail @ ..] => {
                // find child with value=head ...
                let position = self.get_or_create_child(head.clone(), update);
                self.children[position].insert(tail, value, hash, update);
            }
        }

        if update {
            self.update_hash()
        }
    }

    fn diff(&self, path: &mut Vec<T>, other: &Node<T>) -> Vec<Diff<T>>
    where
        T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Ord + Serialize,
    {
        let mut diffs: Vec<Diff<T>> = Vec::new();

        let self_children = self.children();
        let other_children = other.children();

        let mut self_iter = self_children.iter();
        let mut other_iter = other_children.iter();

        let mut self_item = self_iter.next();
        let mut other_item = other_iter.next();

        loop {
            match (self_item, other_item) {
                (Some(self_node), Some(other_node)) => match self_node.value.cmp(&other_node.value)
                {
                    Ordering::Equal => {
                        if self_node.hash != other_node.hash {
                            path.push(self_node.value.clone());
                            let mut children_diff = self_node.diff(path, other_node);
                            path.pop();

                            // if there are no diffs in the children, it likely means that the tree is
                            // cut off further down. In that case we will mark the bottom-most node
                            // as having a hash mismatch. This will propagate upwards, so only this node
                            // will have this diff.
                            if children_diff.is_empty() {
                                diffs.push(Diff::HashMismatch(path.clone()));
                            } else {
                                // otherwise just add the (more specific) diffs from the children
                                diffs.append(&mut children_diff);
                            }
                        }

                        self_item = self_iter.next();
                        other_item = other_iter.next();
                    }
                    Ordering::Less => {
                        path.push(self_node.value.clone());
                        diffs.push(Diff::MissingOther(path.clone()));
                        self_item = self_iter.next();
                        path.pop();
                    }
                    Ordering::Greater => {
                        path.push(other_node.value.clone());
                        diffs.push(Diff::MissingSelf(path.clone()));
                        other_item = other_iter.next();
                        path.pop();
                    }
                },
                (Some(left_node), None) => {
                    path.push(left_node.value.clone());
                    diffs.push(Diff::MissingOther(path.clone()));
                    self_item = self_iter.next();
                    path.pop();
                }
                (None, Some(right_node)) => {
                    path.push(right_node.value.clone());
                    diffs.push(Diff::MissingSelf(path.clone()));
                    other_item = other_iter.next();
                    path.pop();
                }
                (None, None) => {
                    break;
                }
            }
        }

        diffs
    }
}

impl<T> Display for TreeSet<T>
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

impl<T> Display for Node<T>
where
    T: AsRef<[u8]> + Sized + Clone + Eq + PartialEq + Default + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}{}", "-".repeat(self.level as usize), self.value).and_then(|_| {
            for child in self.children.iter() {
                write!(f, "{}", child)?;
            }

            Ok(())
        })
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
enum Diff<T> {
    MissingSelf(Vec<T>),
    MissingOther(Vec<T>),
    HashMismatch(Vec<T>),
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, U256};
    use rand::thread_rng;
    use rand::Rng;

    use crate::types::{PremintMetadata, PremintName};

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_tree_diff() {
        let mut set1 = TreeSet::new();
        let mut set2 = TreeSet::new();

        set1.insert(&["a", "b", "c"], "1");
        set1.insert(&["a", "b", "d"], "2");
        set1.insert(&["a", "b", "e"], "3");
        set1.insert(&["a", "b", "f"], "4");
        set1.insert(&["a", "b", "g"], "5");
        set1.insert_with_hash(&["a", "b", "i"], "6", 10);

        set2.insert(&["a", "b", "c"], "1");
        set2.insert(&["a", "b", "d"], "2");
        set2.insert(&["a", "b", "e"], "3");
        set2.insert(&["a", "b", "f"], "4");
        set2.insert(&["a", "b", "h"], "5");
        set2.insert_with_hash(&["a", "b", "i"], "6", 5);

        set1.rehash();
        set2.rehash();

        // diff with self should be empty
        assert_eq!(set1.diff(&set1).len(), 0);
        assert_eq!(set2.diff(&set2).len(), 0);

        let d = set1.diff(&set2);
        assert_eq!(
            d,
            vec![
                Diff::MissingOther(vec!["a", "b", "g"]),
                Diff::MissingSelf(vec!["a", "b", "h"]),
                Diff::HashMismatch(vec!["a", "b", "i"]),
            ]
        );
    }

    #[test_log::test(tokio::test)]
    async fn test_tree_leafs() {
        let mut set1 = TreeSet::new();

        set1.insert(&["a", "b", "c"], "1");
        set1.insert(&["a", "b", "d"], "2");
        set1.insert(&["a", "b", "e"], "3");
        set1.insert(&["a", "b", "f"], "4");
        set1.insert(&["a", "b", "g"], "5");

        let leafs = set1.leafs(&[]);

        assert_eq!(leafs, vec!["1", "2", "3", "4", "5"]);
    }

    #[test_log::test(tokio::test)]
    async fn test_large_tree() {
        let mut set1 = TreeSet::new();
        let one_hundred = U256::from(100);

        // this is bigger than we would expect our real data to be, because the random token_ids
        // are much larger than our average token_id. so as long as this test is passing we should
        // be good.
        fn random_metadata() -> PremintMetadata {
            let chain_id = thread_rng().gen_range(1..=5);
            let collection_address = Address::new(thread_rng().gen::<[u8; 20]>());
            let signer = Address::new(thread_rng().gen::<[u8; 20]>());
            let token_id = U256::from_be_bytes(thread_rng().gen::<[u8; U256::BYTES]>());
            let version = thread_rng().gen_range(1..=10);
            let id = format!(
                "{}-{}-{}-{}",
                &chain_id, &collection_address, &token_id, &version
            );
            let uri = "".to_string();
            let kind = PremintName("Test".to_string());

            PremintMetadata {
                chain_id,
                version,
                token_id,
                collection_address,
                id,
                signer,
                uri,
                kind,
            }
        }

        for _i in 0..100000 {
            let metadata = random_metadata();
            let collection_address = metadata.collection_address.to_string().to_lowercase();
            let token_id = metadata.token_id;

            let path = [
                metadata.kind.0,
                metadata.chain_id.to_string(),
                collection_address[2..4].to_string(),
                collection_address[4..6].to_string(),
                collection_address.to_string(),
                format!("{:0>2}", token_id.reduce_mod(one_hundred).to::<u64>()),
            ];

            set1.insert(&path, metadata.id);
        }

        set1.rehash();

        // extract all nodes with kind=Test and chain_id=1
        // we'd expect nodes to be mainly interested in specific premint types on specific chains
        let extracted = set1.extract(&["Test", "1"].map(ToString::to_string), 5);
        let serialized = serde_cbor::to_vec(extracted.root()).unwrap();

        tracing::info!("serialized tree is {} bytes", serialized.len());

        assert!(
            serialized.len() < 1024 * 1024 * 10,
            "serialized tree is too large ({} bytes)",
            serialized.len()
        );
    }
}
