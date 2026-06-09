extends SceneTree

const MIN_WALK_DISTANCE := 500.0
const MIN_AVG_FPS := 55.0
const MIN_MIN_FPS := 30.0

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var mesh_counts := _count_mesh_data_vegetation()
	if mesh_counts["ground"] <= 0:
		_fail("missing_vegetation_ground_mesh_data")
		return
	if mesh_counts["tree"] <= 0 or mesh_counts["conifer"] <= 0 or mesh_counts["shrub"] <= 0:
		_fail("missing_vegetation_profiles_in_mesh_data")
		return

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

	var player: CharacterBody3D = scene.get_node_or_null("Player")
	if player == null:
		_fail("missing_player")
		return
	var streamer: Node = scene.get_node_or_null("WorldStreamer")
	if streamer == null:
		_fail("missing_world_streamer")
		return
	var first_position: Vector3 = mesh_counts["first_position"]
	player.global_position = first_position + Vector3(0.0, 1.0, 0.0)
	if streamer.has_method("_refresh_streaming"):
		streamer.call("_refresh_streaming")

	await _wait_for_streamed_geometry(streamer)

	var runtime_counts := _count_runtime_vegetation(scene)
	var perf := await _walk_player_and_measure_fps(player, streamer, MIN_WALK_DISTANCE)

	print("VEGETATION_E2E mesh_ground=", mesh_counts["ground"])
	print("VEGETATION_E2E mesh_tree=", mesh_counts["tree"])
	print("VEGETATION_E2E mesh_conifer=", mesh_counts["conifer"])
	print("VEGETATION_E2E mesh_shrub=", mesh_counts["shrub"])
	print("VEGETATION_E2E runtime_markers=", runtime_counts["markers"])
	print("VEGETATION_E2E runtime_batches=", runtime_counts["batches"])
	print("VEGETATION_E2E walk_distance=", perf["walk_distance"])
	print("VEGETATION_E2E avg_fps=", perf["avg_fps"])
	print("VEGETATION_E2E min_fps=", perf["min_fps"])

	var failed := false
	if runtime_counts["markers"] <= 0:
		push_error("VEGETATION_E2E no runtime vegetation metadata marker")
		failed = true
	if runtime_counts["batches"] <= 0:
		push_error("VEGETATION_E2E no runtime vegetation batch")
		failed = true
	if perf["walk_distance"] < MIN_WALK_DISTANCE:
		push_error("VEGETATION_E2E player did not walk far enough")
		failed = true
	if perf["avg_fps"] < MIN_AVG_FPS:
		push_error("VEGETATION_E2E average FPS below threshold")
		failed = true
	if perf["min_fps"] < MIN_MIN_FPS:
		push_error("VEGETATION_E2E minimum FPS below threshold")
		failed = true

	quit(1 if failed else 0)

func _count_mesh_data_vegetation() -> Dictionary:
	var counts := {
		"ground": 0,
		"tree": 0,
		"conifer": 0,
		"shrub": 0,
		"first_position": Vector3.ZERO,
	}
	var origins := _mesh_data_origins_by_path()
	var dir := DirAccess.open("res://mesh_data")
	if dir == null:
		return counts
	dir.list_dir_begin()
	while true:
		var file_name := dir.get_next()
		if file_name.is_empty():
			break
		if dir.current_is_dir() or not file_name.ends_with(".json"):
			continue
		var file := FileAccess.open("res://mesh_data/" + file_name, FileAccess.READ)
		if file == null:
			continue
		var parsed: Variant = JSON.parse_string(file.get_as_text())
		if typeof(parsed) != TYPE_DICTIONARY:
			continue
		for element in parsed.get("elements", []):
			if typeof(element) != TYPE_DICTIONARY:
				continue
			var name := str(element.get("name", ""))
			if name.begins_with("VegetationGround_"):
				counts["ground"] = int(counts["ground"]) + 1
				if counts["first_position"] == Vector3.ZERO:
					var origin: Vector2 = origins.get("res://mesh_data/" + file_name, Vector2.ZERO)
					var transform: Array = element.get("transform", [])
					if transform.size() >= 12:
						counts["first_position"] = Vector3(
							origin.x + float(transform[9]),
							0.0,
							origin.y + float(transform[11])
						)
			elif name.begins_with("VegetationTree_"):
				counts["tree"] = int(counts["tree"]) + 1
			elif name.begins_with("VegetationConifer_"):
				counts["conifer"] = int(counts["conifer"]) + 1
			elif name.begins_with("VegetationShrub_"):
				counts["shrub"] = int(counts["shrub"]) + 1
	return counts

func _mesh_data_origins_by_path() -> Dictionary:
	var origins := {}
	var file := FileAccess.open("res://world_manifest.json", FileAccess.READ)
	if file == null:
		return origins
	var parsed: Variant = JSON.parse_string(file.get_as_text())
	if typeof(parsed) != TYPE_DICTIONARY:
		return origins
	for chunk in parsed.get("chunks", []):
		if typeof(chunk) != TYPE_DICTIONARY:
			continue
		var path := str(chunk.get("mesh_data_path", ""))
		var origin: Array = chunk.get("origin", [])
		if not path.is_empty() and origin.size() >= 2:
			origins[path] = Vector2(float(origin[0]), float(origin[1]))
	return origins

func _wait_for_streamed_geometry(streamer: Node) -> void:
	for i in range(600):
		var counts := _count_runtime_vegetation(streamer)
		if int(counts["batches"]) > 0 and int(counts["markers"]) > 0:
			return
		await process_frame
		await physics_frame

func _count_runtime_vegetation(node: Node) -> Dictionary:
	var counts := {
		"markers": 0,
		"batches": 0,
	}
	_accumulate_runtime_vegetation(node, counts)
	return counts

func _accumulate_runtime_vegetation(node: Node, counts: Dictionary) -> void:
	if node.has_meta("osm_kind") and str(node.get_meta("osm_kind")) == "vegetation":
		counts["markers"] = int(counts["markers"]) + 1
	if node is MeshInstance3D and node.has_meta("batch_material"):
		var material := str(node.get_meta("batch_material"))
		if material == "tree_leaves" or material == "terrain_grass":
			counts["batches"] = int(counts["batches"]) + 1
	for child in node.get_children():
		_accumulate_runtime_vegetation(child, counts)

func _walk_player_and_measure_fps(player: CharacterBody3D, streamer: Node, distance: float) -> Dictionary:
	var start := player.global_position
	var direction := Vector3.RIGHT
	var walked := 0.0
	var total_fps := 0.0
	var min_fps := INF
	var samples := 0
	var step := 5.0

	while walked < distance:
		var before := Time.get_ticks_usec()
		player.global_position = start + direction * walked
		if streamer.has_method("_refresh_streaming"):
			streamer.call("_refresh_streaming")
		await process_frame
		await physics_frame
		var elapsed: int = max(Time.get_ticks_usec() - before, 1)
		var fps: float = clamp(1000000.0 / float(elapsed), 0.0, 1000.0)
		total_fps += fps
		min_fps = min(min_fps, fps)
		samples += 1
		walked += step

	player.global_position = start + direction * distance
	if streamer.has_method("_refresh_streaming"):
		streamer.call("_refresh_streaming")
	await process_frame

	return {
		"walk_distance": distance,
		"avg_fps": total_fps / max(float(samples), 1.0),
		"min_fps": min_fps,
	}

func _fail(reason: String) -> void:
	push_error("VEGETATION_E2E " + reason)
	quit(1)
