use raven_core::container::TempList;

#[test]
pub fn test_temp_list_add() {
    let temp_list = TempList::new();

    let refers = (0..2000).into_iter()
        .map(|i| temp_list.add(i))
        .collect::<Vec<_>>();
    
    let mut index = 0; 
    for reference in refers {
        assert_eq!(index, *reference);
        index += 1;
    }
}