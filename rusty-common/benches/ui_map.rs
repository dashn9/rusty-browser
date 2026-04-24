use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rusty_common::ui_map::{UiNode, diff, format_compact};

fn make_nodes(n: usize) -> Vec<UiNode> {
    (1..=n as i64)
        .map(|i| UiNode {
            id: i,
            role: "button".to_string(),
            name: Some(format!("node-{i}")),
            parent_id: if i > 1 { Some(i - 1) } else { None },
            value: Some(format!("val-{i}")),
            properties: None,
        })
        .collect()
}

fn bench_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("ui_map/diff");

    for size in [10, 100, 500, 1000] {
        let before = make_nodes(size);
        // after: first half unchanged, second half replaced with new ids
        let mut after = before[..size / 2].to_vec();
        after.extend(make_nodes(size / 2).into_iter().map(|mut n| {
            n.id += size as i64 * 2; // new ids → all added
            n
        }));

        group.bench_with_input(BenchmarkId::new("size", size), &(before, after), |b, (bef, aft)| {
            b.iter(|| diff(black_box(bef), black_box(aft)));
        });
    }
    group.finish();
}

fn bench_diff_no_changes(c: &mut Criterion) {
    let mut group = c.benchmark_group("ui_map/diff_identical");

    for size in [10, 100, 500, 1000] {
        let nodes = make_nodes(size);
        group.bench_with_input(BenchmarkId::new("size", size), &nodes, |b, nodes| {
            b.iter(|| diff(black_box(nodes), black_box(nodes)));
        });
    }
    group.finish();
}

fn bench_diff_all_changed(c: &mut Criterion) {
    let mut group = c.benchmark_group("ui_map/diff_all_changed");

    for size in [10, 100, 500] {
        let before = make_nodes(size);
        let after: Vec<UiNode> = before
            .iter()
            .map(|n| UiNode {
                id: n.id,
                role: n.role.clone(),
                name: n.name.clone(),
                value: Some("changed".to_string()),
                parent_id: n.parent_id,
                properties: None,
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("size", size), &(before, after), |b, (bef, aft)| {
            b.iter(|| diff(black_box(bef), black_box(aft)));
        });
    }
    group.finish();
}

fn bench_format_compact(c: &mut Criterion) {
    let mut group = c.benchmark_group("ui_map/format_compact");

    for size in [10, 100, 500, 1000] {
        let nodes = make_nodes(size);
        group.bench_with_input(BenchmarkId::new("size", size), &nodes, |b, nodes| {
            b.iter(|| format_compact(black_box(nodes)));
        });
    }
    group.finish();
}

fn bench_format_compact_no_optionals(c: &mut Criterion) {
    let nodes: Vec<UiNode> = (1..=500i64)
        .map(|i| UiNode {
            id: i,
            role: "div".to_string(),
            name: None,
            parent_id: None,
            value: None,
            properties: None,
        })
        .collect();

    c.bench_function("ui_map/format_compact_no_optionals/500", |b| {
        b.iter(|| format_compact(black_box(&nodes)));
    });
}

fn bench_diff_with_zero_id_nodes(c: &mut Criterion) {
    // Zero-id nodes must be filtered — measure the overhead
    let mut before = make_nodes(200);
    before.extend((0..50).map(|_| UiNode {
        id: 0,
        role: "ignored".to_string(),
        name: None,
        parent_id: None,
        value: None,
        properties: None,
    }));
    let after = before.clone();

    c.bench_function("ui_map/diff_with_zero_ids/250", |b| {
        b.iter(|| diff(black_box(&before), black_box(&after)));
    });
}

criterion_group!(
    benches,
    bench_diff,
    bench_diff_no_changes,
    bench_diff_all_changed,
    bench_format_compact,
    bench_format_compact_no_optionals,
    bench_diff_with_zero_id_nodes,
);
criterion_main!(benches);
