pub fn solve() {
    cp::prepare!();
    sc!(n: usize);

    let mut pointer = n >> 1;
    let nums: Vec<usize> = (1..=n).collect();
    let (mut left_sum, mut right_sum) = (0usize, 0usize);
    let (mut start, mut end) = (0usize, n);

    loop {
        left_sum = nums[..=pointer].iter().sum();
        right_sum = nums[(pointer + 1)..n].iter().sum();
        if left_sum == left_sum {
            break;
        } else if left_sum < right_sum {
            start = pointer + 1;
            pointer = (start + end) >> 2;
        } else if left_sum > right_sum {
            end = pointer;
            pointer = (start + end) >> 2;
        }
    }

    pp!(pointer + 1);
    pp!(@it nums[..=pointer]);
    pp!(n - pointer - 1);
    pp!(@it nums[(pointer + 1)..n]);
    // pp!()
}

cp::main!();
