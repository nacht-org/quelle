use std::sync::Arc;

use ego_tree::NodeId;
use html5ever::{LocalName, QualName, ns};
use wasmtime::component::{HasData, Resource, ResourceTable};

use crate::bindings::quelle::extension::scraper as wit;
use crate::state::State;

/// Newtype that makes `scraper::Html` usable inside `Arc` for async wasmtime hosts.
///
/// # Safety
/// `scraper::Html` is `!Send` due to internal `Cell` usage. All access is
/// serialised through the wasmtime store lock, so only one thread touches the
/// data at a time.
struct HtmlTree(scraper::Html);

unsafe impl Send for HtmlTree {}
unsafe impl Sync for HtmlTree {}

/// Parsed HTML document. The tree is `Arc`-shared so node handles can outlive it.
pub struct HostDocument {
    tree: Arc<HtmlTree>,
}

/// Handle to an element node — shared tree reference plus a stable `NodeId`.
pub struct HostNode {
    tree: Arc<HtmlTree>,
    id: NodeId,
}

/// Handle to a text node. Same layout as `HostNode` but without tag or attributes.
pub struct HostTextNode {
    tree: Arc<HtmlTree>,
    id: NodeId,
}

pub struct Scraper {
    table: ResourceTable,
}

impl Scraper {
    pub fn new() -> Self {
        Self {
            table: ResourceTable::new(),
        }
    }
}

impl HasData for Scraper {
    type Data<'a> = &'a mut State;
}

fn compile_selector(selector: &str) -> Result<scraper::Selector, wit::SelectorError> {
    scraper::Selector::parse(selector).map_err(|e| wit::SelectorError {
        message: format!("invalid selector `{selector}`: {e}"),
    })
}

impl wit::HostDocument for Scraper {
    async fn new(&mut self, html: String) -> Resource<HostDocument> {
        let tree = Arc::new(HtmlTree(scraper::Html::parse_document(&html)));
        self.table
            .push(HostDocument { tree })
            .expect("resource table push failed")
    }

    async fn select(
        &mut self,
        self_: Resource<HostDocument>,
        selector: String,
    ) -> Result<Vec<Resource<HostNode>>, wit::SelectorError> {
        let compiled = compile_selector(&selector)?;
        let doc = self.table.get(&self_).expect("document resource missing");
        let tree = Arc::clone(&doc.tree);

        let ids: Vec<NodeId> = tree.0.select(&compiled).map(|el| el.id()).collect();

        Ok(ids
            .into_iter()
            .map(|id| {
                self.table
                    .push(HostNode {
                        tree: Arc::clone(&tree),
                        id,
                    })
                    .expect("resource table push failed")
            })
            .collect())
    }

    async fn select_first(
        &mut self,
        self_: Resource<HostDocument>,
        selector: String,
    ) -> Result<Option<Resource<HostNode>>, wit::SelectorError> {
        let compiled = compile_selector(&selector)?;
        let doc = self.table.get(&self_).expect("document resource missing");
        let tree = Arc::clone(&doc.tree);

        let id = tree.0.select(&compiled).next().map(|el| el.id());

        Ok(id.map(|id| {
            self.table
                .push(HostNode {
                    tree: Arc::clone(&tree),
                    id,
                })
                .expect("resource table push failed")
        }))
    }

    async fn drop(&mut self, rep: Resource<HostDocument>) -> wasmtime::Result<()> {
        let _ = self.table.delete(rep)?;
        Ok(())
    }
}

impl wit::HostNode for Scraper {
    async fn select(
        &mut self,
        self_: Resource<HostNode>,
        selector: String,
    ) -> Result<Vec<Resource<HostNode>>, wit::SelectorError> {
        let compiled = compile_selector(&selector)?;
        let node = self.table.get(&self_).expect("node resource missing");
        let tree = Arc::clone(&node.tree);
        let node_id = node.id;

        let element_ref =
            scraper::ElementRef::wrap(tree.0.tree.get(node_id).expect("node id not found in tree"))
                .expect("node is not an element");

        let ids: Vec<NodeId> = element_ref.select(&compiled).map(|el| el.id()).collect();

        Ok(ids
            .into_iter()
            .map(|id| {
                self.table
                    .push(HostNode {
                        tree: Arc::clone(&tree),
                        id,
                    })
                    .expect("resource table push failed")
            })
            .collect())
    }

    async fn select_first(
        &mut self,
        self_: Resource<HostNode>,
        selector: String,
    ) -> Result<Option<Resource<HostNode>>, wit::SelectorError> {
        let compiled = compile_selector(&selector)?;
        let node = self.table.get(&self_).expect("node resource missing");
        let tree = Arc::clone(&node.tree);
        let node_id = node.id;

        let element_ref =
            scraper::ElementRef::wrap(tree.0.tree.get(node_id).expect("node id not found in tree"))
                .expect("node is not an element");

        let id = element_ref.select(&compiled).next().map(|el| el.id());

        Ok(id.map(|id| {
            self.table
                .push(HostNode {
                    tree: Arc::clone(&tree),
                    id,
                })
                .expect("resource table push failed")
        }))
    }

    async fn name(&mut self, self_: Resource<HostNode>) -> String {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .map(|el| el.value().name().to_string())
            .unwrap_or_default()
    }

    async fn attr(&mut self, self_: Resource<HostNode>, name: String) -> Option<String> {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .and_then(|el| el.value().attr(&name))
            .map(|s| s.to_string())
    }

    async fn text(&mut self, self_: Resource<HostNode>) -> String {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
    }

    async fn has_attr(&mut self, self_: Resource<HostNode>, name: String) -> bool {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .and_then(|el| el.value().attr(&name))
            .is_some()
    }

    async fn attr_names(&mut self, self_: Resource<HostNode>) -> Vec<String> {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .map(|el| {
                el.value()
                    .attrs()
                    .map(|(name, _)| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn remove_attr(&mut self, self_: Resource<HostNode>, name: String) {
        let node = self.table.get(&self_).expect("node resource missing");
        let node_id = node.id;
        // Safety: all tree mutations are serialised through the wasmtime store lock.
        let tree_ptr = Arc::as_ptr(&node.tree) as *mut HtmlTree;
        unsafe {
            if let Some(mut n) = (*tree_ptr).0.tree.get_mut(node_id) {
                if let scraper::node::Node::Element(el) = n.value() {
                    let qualname = QualName::new(None, ns!(), LocalName::from(name.as_str()));
                    if let Ok(idx) = el.attrs.binary_search_by(|a| a.0.cmp(&qualname)) {
                        el.attrs.remove(idx);
                    }
                }
            }
        }
    }

    async fn outer_html(&mut self, self_: Resource<HostNode>) -> String {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .map(|el| el.html())
            .unwrap_or_default()
    }

    async fn inner_html(&mut self, self_: Resource<HostNode>) -> String {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("node id not found in tree");
        scraper::ElementRef::wrap(tree_node)
            .map(|el| el.inner_html())
            .unwrap_or_default()
    }

    async fn detach(&mut self, self_: Resource<HostNode>) {
        let node = self.table.get(&self_).expect("node resource missing");
        let node_id = node.id;
        // Safety: all tree mutations are serialised through the wasmtime store lock.
        let tree_ptr = Arc::as_ptr(&node.tree) as *mut HtmlTree;
        unsafe {
            if let Some(mut n) = (*tree_ptr).0.tree.get_mut(node_id) {
                n.detach();
            }
        }
    }

    async fn children(&mut self, self_: Resource<HostNode>) -> Vec<wit::ChildNode> {
        let node = self.table.get(&self_).expect("node resource missing");
        let tree = Arc::clone(&node.tree);
        let node_id = node.id;

        let child_info: Vec<(NodeId, bool)> = tree
            .0
            .tree
            .get(node_id)
            .expect("node id not found in tree")
            .children()
            .filter_map(|n| {
                let is_element = n.value().is_element();
                let is_text = n.value().is_text();
                if is_element || is_text {
                    Some((n.id(), is_element))
                } else {
                    None
                }
            })
            .collect();

        child_info
            .into_iter()
            .map(|(id, is_element)| {
                if is_element {
                    wit::ChildNode::Element(
                        self.table
                            .push(HostNode {
                                tree: Arc::clone(&tree),
                                id,
                            })
                            .expect("resource table push failed"),
                    )
                } else {
                    wit::ChildNode::Text(
                        self.table
                            .push(HostTextNode {
                                tree: Arc::clone(&tree),
                                id,
                            })
                            .expect("resource table push failed"),
                    )
                }
            })
            .collect()
    }

    async fn drop(&mut self, rep: Resource<HostNode>) -> wasmtime::Result<()> {
        let _ = self.table.delete(rep)?;
        Ok(())
    }
}

impl wit::HostTextNode for Scraper {
    async fn text(&mut self, self_: Resource<HostTextNode>) -> String {
        let node = self.table.get(&self_).expect("text-node resource missing");
        let tree_node = node
            .tree
            .0
            .tree
            .get(node.id)
            .expect("text node id not found in tree");
        match tree_node.value() {
            scraper::node::Node::Text(t) => t.text.to_string(),
            _ => String::new(),
        }
    }

    async fn set_text(&mut self, self_: Resource<HostTextNode>, content: String) {
        let node = self.table.get(&self_).expect("text-node resource missing");
        let node_id = node.id;
        // Safety: all tree mutations are serialised through the wasmtime store lock.
        let tree_ptr = Arc::as_ptr(&node.tree) as *mut HtmlTree;
        unsafe {
            if let Some(mut n) = (*tree_ptr).0.tree.get_mut(node_id) {
                if let scraper::node::Node::Text(t) = n.value() {
                    t.text = content.into();
                }
            }
        }
    }

    async fn drop(&mut self, rep: Resource<HostTextNode>) -> wasmtime::Result<()> {
        let _ = self.table.delete(rep)?;
        Ok(())
    }
}

impl wit::Host for Scraper {}
