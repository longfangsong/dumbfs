#[cfg(test)]
pub fn create_test_img() {
    use std::rc::Rc;
    use std::cell::RefCell;
    use std::fs::OpenOptions;
    use crate::fs::meta::DumbFsMeta;
    use std::ops::DerefMut;
    use crate::file_meta::File;
    use fuse::FileType::RegularFile;
    let f = Rc::new(RefCell::new(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("/tmp/test.img")
        .unwrap()));
    let mut fsmeta = DumbFsMeta::new();
    fsmeta.serialize_into((*f).borrow_mut().deref_mut()).unwrap();

    let mut root = File::new(fsmeta.next_free_address(), f.clone());
    let next_ino = fsmeta.acquire_next_ino((*f).borrow_mut().deref_mut());
    root.set_ino(next_ino);
    root.set_name("");
    root.set_content(&[]);
    fsmeta.sync((*f).borrow_mut().deref_mut());

    let mut dir1 = File::new(fsmeta.next_free_address(), f.clone());
    let next_ino = fsmeta.acquire_next_ino((*f).borrow_mut().deref_mut());
    dir1.set_ino(next_ino);
    dir1.set_name("dir1");
    dir1.set_content(&[]);
    fsmeta.sync((*f).borrow_mut().deref_mut());
    root.set_first_child(dir1.address());

    let mut dir2 = File::new(fsmeta.next_free_address(), f.clone());
    let next_ino = fsmeta.acquire_next_ino((*f).borrow_mut().deref_mut());
    let mut header = root.header();
    header.fixed_sized_part.file_attr.ino = next_ino;
    header.filename = "dir2".to_string();
    dir2.set_ino(next_ino);
    dir2.set_name("dir2");
    dir2.set_content(&[]);
    fsmeta.sync((*f).borrow_mut().deref_mut());
    dir1.set_next_sibling(dir2.address());

    let mut file1 = File::new(fsmeta.next_free_address(), f.clone());
    let next_ino = fsmeta.acquire_next_ino((*f).borrow_mut().deref_mut());
    file1.set_ino(next_ino);
    file1.set_file_type(RegularFile);
    file1.set_name("hello.txt");
    file1.set_content(b"hello world\n");
    fsmeta.sync((*f).borrow_mut().deref_mut());
    dir2.set_next_sibling(file1.address());

    let mut file2 = File::new(fsmeta.next_free_address(), f.clone());
    let next_ino = fsmeta.acquire_next_ino((*f).borrow_mut().deref_mut());
    file2.set_ino(next_ino);
    file2.set_file_type(RegularFile);
    file2.set_name("bye.txt");
    file2.set_content(b"goodbye world\n");
    fsmeta.sync((*f).borrow_mut().deref_mut());
    file1.set_next_sibling(file2.address());
}

#[test]
fn f() {
    create_test_img();
}