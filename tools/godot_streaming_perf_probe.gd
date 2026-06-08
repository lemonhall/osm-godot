extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var packed: PackedScene = load("res://scenes/master.tscn")
	if packed == null:
		_fail("failed_to_load_master")
		return

	var scene: Node = packed.instantiate()
	get_root().add_child(scene)
	current_scene = scene

	for i in range(16):
		await process_frame
		await physics_frame

	var streamer: Node = scene.get_node_or_null("WorldStreamer")
	if streamer == null:
		_fail("missing_world_streamer")
		return

	var loaded_chunks: Dictionary = streamer.get("loaded_chunks")
	var counts: Dictionary = _count_streamed_nodes(streamer)
	var refresh_usec: float = _measure_refresh_usec(streamer, 300)
	var max_mesh_instances: int = max(loaded_chunks.size() * 64, 1)

	print("STREAM_PERF loaded_chunks=", loaded_chunks.size())
	print("STREAM_PERF mesh_instances=", counts["mesh_instances"])
	print("STREAM_PERF batch_element_total=", counts["batch_element_total"])
	print("STREAM_PERF metadata_nodes=", counts["metadata_nodes"])
	print("STREAM_PERF avg_refresh_usec=", refresh_usec)
	print("STREAM_PERF max_mesh_instances=", max_mesh_instances)

	var failed := false
	if loaded_chunks.is_empty():
		push_error("STREAM_PERF no chunks loaded")
		failed = true
	if int(counts["mesh_instances"]) > max_mesh_instances:
		push_error("STREAM_PERF mesh instance count is not batched enough")
		failed = true
	if int(counts["batch_element_total"]) <= int(counts["mesh_instances"]):
		push_error("STREAM_PERF batch element total did not exceed mesh instances")
		failed = true
	if refresh_usec > 1000.0:
		push_error("STREAM_PERF steady-state refresh is too slow")
		failed = true

	quit(1 if failed else 0)

func _count_streamed_nodes(node: Node) -> Dictionary:
	var counts: Dictionary = {
		"mesh_instances": 0,
		"batch_element_total": 0,
		"metadata_nodes": 0,
	}
	_accumulate_counts(node, counts)
	return counts

func _accumulate_counts(node: Node, counts: Dictionary) -> void:
	if node is MeshInstance3D:
		counts["mesh_instances"] = int(counts["mesh_instances"]) + 1
		if node.has_meta("batch_element_count"):
			counts["batch_element_total"] = int(counts["batch_element_total"]) + int(node.get_meta("batch_element_count"))
	if node.has_meta("osm_id") and node.has_meta("osm_kind"):
		counts["metadata_nodes"] = int(counts["metadata_nodes"]) + 1
	for child: Node in node.get_children():
		_accumulate_counts(child, counts)

func _measure_refresh_usec(streamer: Node, iterations: int) -> float:
	var callable := Callable(streamer, "_refresh_streaming")
	var start: int = Time.get_ticks_usec()
	for i in range(iterations):
		callable.call()
	var elapsed: int = Time.get_ticks_usec() - start
	return float(elapsed) / float(iterations)

func _fail(reason: String) -> void:
	push_error("STREAM_PERF " + reason)
	quit(1)
