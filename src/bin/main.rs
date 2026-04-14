// using iterators to solve. @it references an interator, and prints it accordingly in the print
// statement.

pub fn solve() {
    my_lib::prepare!();
    sc!(n, mut a: [usize; n]);
    a.sort_unstable();
    let mut ans = vec![];
    let check = |a: &[usize]| {
        let n = a.len();
        n % 2 == 0 && (0..n).all(|i| a[i] + a[n - i - 1] == a[0] + a[n - 1])
    };
    if check(&a) {
        ans.push(a[0] + a[n - 1]);
    }
    let l = a[n - 1];
    while a.last() == Some(&l) {
        a.pop();
    }
    if check(&a) {
        ans.push(l);
    }
    ans.sort_unstable();
    pp!(@it ans);
}

my_lib::main!();
