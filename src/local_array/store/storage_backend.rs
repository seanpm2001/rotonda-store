use crossbeam_epoch::Guard;

use routecore::record::{MergeUpdate, Meta};

use crate::af::AddressFamily;
use crate::custom_alloc::{PrefixBuckets, MoreSpecificsPrefixIter, PrefixSet, LessSpecificPrefixIter};
use crate::local_array::bit_span::BitSpan;
use crate::prefix_record::InternalPrefixRecord;
use crate::{custom_alloc::StoredPrefix, local_array::tree::*};

pub(crate) type SizedNodeRefOption<'a, AF> = Option<SizedStrideRef<'a, AF>>;

pub trait StorageBackend {
    type AF: AddressFamily;
    type Meta: Meta + MergeUpdate;
    type PB: PrefixBuckets<Self::AF, Self::Meta>;

    fn init(root_node: SizedStrideNode<Self::AF>) -> Self;

    //-------- Nodes --------------------------------------------------------
    fn acquire_new_node_id(
        &self,
        sub_prefix: (Self::AF, u8),
    ) -> StrideNodeId<Self::AF>;
    fn store_node(
        &self,
        id: StrideNodeId<Self::AF>,
        next_node: SizedStrideNode<Self::AF>,
    ) -> Option<StrideNodeId<Self::AF>>;
    fn update_node(
        &self,
        current_node_id: StrideNodeId<Self::AF>,
        updated_node: SizedStrideRefMut<Self::AF>,
    );
    fn retrieve_node_mut_with_guard<'a>(
        &'a self,
        id: StrideNodeId<Self::AF>,
        guard: &'a Guard,
    ) -> Option<SizedStrideRefMut<'a, Self::AF>>;
    fn retrieve_node_with_guard<'a>(
        &'a self,
        id: StrideNodeId<Self::AF>,
        guard: &'a Guard,
    ) -> Option<SizedStrideRef<'a, Self::AF>>;
    fn store_node_with_guard<'a>(
        &'a self,
        current_node: SizedNodeRefOption<'a, Self::AF>,
        next_node: SizedStrideNode<Self::AF>,
        guard: &'a Guard,
    ) -> Option<StrideNodeId<Self::AF>>;
    fn get_root_node_id(&self) -> StrideNodeId<Self::AF>;
    fn get_node_id_for_prefix(&self, prefix: &PrefixId<Self::AF>) -> (StrideNodeId<Self::AF>, BitSpan);
    fn load_default_route_prefix_serial(&self) -> usize;
    fn increment_default_route_prefix_serial(&self) -> usize;
    fn get_nodes_len(&self) -> usize;
    fn acquire_new_prefix_id(
        &self,
        prefix: &InternalPrefixRecord<Self::AF, Self::Meta>,
    ) -> PrefixId<Self::AF>;
    fn store_prefix(
        &self,
        id: PrefixId<Self::AF>,
        node: InternalPrefixRecord<Self::AF, Self::Meta>,
        serial: usize,
    ) -> Option<PrefixId<Self::AF>>;
    fn upsert_prefix(
        &self,
        pfx_rec: InternalPrefixRecord<Self::AF, Self::Meta>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn retrieve_prefix(
        &self,
        index: PrefixId<Self::AF>,
    ) -> Option<InternalPrefixRecord<Self::AF, Self::Meta>>;

    #[allow(clippy::type_complexity)]
    fn retrieve_prefix_with_guard<'a>(
        &'a self,
        id: PrefixId<Self::AF>,
        guard: &'a Guard,
    ) -> Option<(&'a InternalPrefixRecord<Self::AF, Self::Meta>, &'a usize)>;

    #[allow(clippy::type_complexity)]
    fn non_recursive_retrieve_prefix_with_guard<'a>(
        &'a self,
        id: PrefixId<Self::AF>,
        guard: &'a Guard,
    ) -> (
        Option<(&InternalPrefixRecord<Self::AF, Self::Meta>, &'a usize)>,
        Option<(
            PrefixId<Self::AF>,
            u8,
            &'a PrefixSet<Self::AF, Self::Meta>,
            [Option<(&'a PrefixSet<Self::AF, Self::Meta>, usize)>; 26],
            usize,
        )>,
    );

    // Retrieves the LOCATION of a prefix as &mut. That means that an empty
    // StoredPrefix may be returned, so that the caller can create a new
    // prefix. This why we need to have a guard passed in as well. This
    // method returns the level at which the prefix was stored as well,
    // so that the caller can create a new prefix at this spot and calculate
    // the correct length of its Next PrefixSet. This method is used by
    // `upsert_prefix`.
    fn retrieve_prefix_mut_with_guard<'a>(
        &'a self,
        id: PrefixId<Self::AF>,
        guard: &'a Guard,
    ) -> (&'a mut StoredPrefix<Self::AF, Self::Meta>, u8);
    fn remove_prefix(
        &mut self,
        index: PrefixId<Self::AF>,
    ) -> Option<InternalPrefixRecord<Self::AF, Self::Meta>>;
    fn get_prefixes_len(&self) -> usize;
    fn get_stride_for_id(&self, id: StrideNodeId<Self::AF>) -> u8;
    fn get_stride_sizes(&self) -> &[u8];

    // These functions are static method, to be able to get these
    // values at instance creation time.
    fn get_strides_len() -> u8;
    fn get_first_stride_size() -> u8;

    fn more_specific_prefix_iter_from<'a>(
        &'a self,
        start_prefix_id: PrefixId<Self::AF>,
        guard: &'a Guard,
    ) -> Option<MoreSpecificsPrefixIter<Self::AF, Self>> where Self: std::marker::Sized;

    fn prefix_iter_to<'a>(
        &'a self,
        start_prefix_id: PrefixId<Self::AF>,
        guard: &'a Guard,
    ) -> Option<LessSpecificPrefixIter<Self::AF, Self::Meta, Self::PB>>;
}
