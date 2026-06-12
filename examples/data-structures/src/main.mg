// data-structures — structs, collections, and pattern matching.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - struct construction `@Name { … }` and field access
//   - the standard vocabulary (map/sum/min/len, abs) over a list
//   - closures `fn(p) => …` and a helper function
//
// Run:  forge run

S Point { x: i32, y: i32 }

f manhattan(p) {
    abs(p.x) + abs(p.y)
}

f main() {
    val pts = [@Point { x: 3, y: -4 }, @Point { x: -1, y: 2 }, @Point { x: 0, y: 5 }]
    val dists = map(pts, fn(p) => manhattan(p))
    f"points={len(pts)}, total_distance={sum(dists)}, closest={min(dists)}"
}
