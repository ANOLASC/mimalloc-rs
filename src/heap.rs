use crate::mimalloc_types::MiHeap;

// Safe delete a heap without freeing any still allocated blocks in that heap.
pub fn mi_heap_delete(heap: *mut MiHeap) {
    //   mi_assert(heap != NULL);
    //   mi_assert(mi_heap_is_initialized(heap));
    //   mi_assert_expensive(mi_heap_is_valid(heap));
    //   if (heap==NULL || !mi_heap_is_initialized(heap)) return;

    //   if (!mi_heap_is_backing(heap)) {
    //     // tranfer still used pages to the backing heap
    //     mi_heap_absorb(heap->tld->heap_backing, heap);
    //   }
    //   else {
    //     // the backing heap abandons its pages
    //     _mi_heap_collect_abandon(heap);
    //   }
    //   mi_assert_internal(heap->page_count==0);
    //   mi_heap_free(heap);
}

// called from `mi_heap_destroy` and `mi_heap_delete` to free the internal heap resources.
fn mi_heap_free(heap: *mut MiHeap) {
    // mi_assert(heap != NULL);
    // mi_assert_internal(mi_heap_is_initialized(heap));
    // if (heap==NULL || !mi_heap_is_initialized(heap)) return;
    // if (mi_heap_is_backing(heap)) return; // dont free the backing heap

    // // reset default
    // if (mi_heap_is_default(heap)) {
    //   _mi_heap_set_default_direct(heap->tld->heap_backing);
    // }

    // // remove ourselves from the thread local heaps list
    // // linear search but we expect the number of heaps to be relatively small
    // mi_heap_t* prev = NULL;
    // mi_heap_t* curr = heap->tld->heaps;
    // while (curr != heap && curr != NULL) {
    //   prev = curr;
    //   curr = curr->next;
    // }
    // mi_assert_internal(curr == heap);
    // if (curr == heap) {
    //   if (prev != NULL) { prev->next = heap->next; }
    //                else { heap->tld->heaps = heap->next; }
    // }
    // mi_assert_internal(heap->tld->heaps != NULL);

    // // and free the used memory
    // mi_free(heap);
}
