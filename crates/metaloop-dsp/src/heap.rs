use std::cmp::PartialOrd;
use std::fmt::Debug;
use std::mem;

// Define a generic heap struct
#[derive(Debug, PartialEq, Clone)]
struct Heap<T: PartialOrd + Debug + Clone> {
    data: Vec<T>,
}

// Helper function to get the parent index of a node
fn up(n: usize) -> usize {
    n / 2
}

// Build the max heap by heapifying from the first parent downwards
fn build_max_heap<T: PartialOrd + Debug + Clone>(arr: Vec<T>) -> Heap<T> {
    let mut heap = Heap { data: arr };
    let first_parent = up(heap.data.len());
    for i in (1..=first_parent).rev() {
        max_heapify(&mut heap, i);
    }
    heap
}

// Recursive function to maintain max heap property
fn max_heapify<T: PartialOrd + Debug + Clone>(heap: &mut Heap<T>, i: usize) {
    let left = 2 * i;
    let right = 2 * i + 1;
    let len = heap.data.len();

    let mut largest = i;

    if right <= len && heap.data[right - 1] > heap.data[left - 1] {
        largest = right;
    } else if left <= len && heap.data[left - 1] > heap.data[i - 1] {
        largest = left;
    }

    if largest != i {
        heap.data.swap(i - 1, largest - 1);
        max_heapify(heap, largest);
    }
}

// Extract the maximum element, maintaining the max heap structure
fn extract_max<T: PartialOrd + Debug + Clone>(heap: &mut Heap<T>) -> Option<T> {
    if heap.data.is_empty() {
        return None;
    }
    let max = heap.data[0].clone();
    heap.data[0] = heap.data.pop().unwrap();
    max_heapify(heap, 1);
    Some(max)
}

// Insert a new element, maintaining the max heap structure
fn insert_new<T: PartialOrd + Debug + Clone>(heap: &mut Heap<T>, value: T) {
    heap.data.push(value);
    let mut i = heap.data.len();
    while i > 1 && heap.data[i - 1] > heap.data[up(i) - 1] {
        heap.data.swap(i - 1, up(i) - 1);
        i = up(i);
    }
}

// Get the maximum element (root) of the heap
fn top<T: PartialOrd + Debug + Clone>(heap: &Heap<T>) -> Option<&T> {
    heap.data.get(0)
}

// Check if the heap is empty
fn is_empty<T: PartialOrd + Debug + Clone>(heap: &Heap<T>) -> bool {
    heap.data.is_empty()
}

// Tests
fn main() {
    // Basic test cases
    let mut heap = build_max_heap(vec![1, 4, 2]);
    assert_eq!(heap.data, vec![4, 1, 2]);

    insert_new(&mut heap, 10);
    assert_eq!(heap.data, vec![10, 4, 2, 1]);

    insert_new(&mut heap, 6);
    assert_eq!(heap.data, vec![10, 6, 2, 1, 4]);

    let mut heap = build_max_heap(vec![1, 6, 3, 9, 10]);
    assert_eq!(heap.data, vec![10, 9, 3, 1, 6]);

    let mut h = build_max_heap(vec![1, 4, 2, 6, 8, 2, 10]);

    assert_eq!(extract_max(&mut h), Some(10));
    assert_eq!(extract_max(&mut h), Some(8));
    assert_eq!(extract_max(&mut h), Some(6));
    assert_eq!(extract_max(&mut h), Some(4));
    assert_eq!(extract_max(&mut h), Some(2));
    assert_eq!(extract_max(&mut h), Some(2));
    assert_eq!(extract_max(&mut h), Some(1));
    assert!(is_empty(&h));
}
