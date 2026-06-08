extends SceneTree

const MIN_NORMAL_MOVE := 4.0

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

	var player: CharacterBody3D = scene.get_node_or_null("Player")
	if player == null:
		_fail("missing_player")
		return
	var streamer: Node = scene.get_node_or_null("WorldStreamer")
	if streamer == null:
		_fail("missing_world_streamer")
		return
	await _wait_for_initial_stream_content(streamer)

	var manifest: Dictionary = streamer.get("manifest")
	var loaded_chunks: Dictionary = streamer.get("loaded_chunks")
	var stream_radius: int = int(streamer.get("stream_radius"))
	var max_loaded := (stream_radius * 2 + 1) * (stream_radius * 2 + 1)
	var manifest_count := int(manifest.get("chunks", []).size())
	var loaded_initial := loaded_chunks.size()
	var initial_keys := _sorted_keys(loaded_chunks)

	print("STREAM_E2E manifest_chunk_count=", manifest_count)
	print("STREAM_E2E stream_radius=", stream_radius)
	print("STREAM_E2E max_loaded_chunks=", max_loaded)
	print("STREAM_E2E loaded_initial=", loaded_initial)
	print("STREAM_E2E initial_keys=", initial_keys)
	print("STREAM_E2E roads_node_exists=", scene.get_node_or_null("Roads") != null)

	var meta_node := _find_first_node_with_osm_meta(streamer)
	var found_meta := meta_node != null
	print("STREAM_E2E osm_meta_node=", "null" if meta_node == null else meta_node.name)
	if meta_node != null:
		print("STREAM_E2E osm_meta_kind=", meta_node.get_meta("osm_kind"))
		print("STREAM_E2E osm_meta_id=", meta_node.get_meta("osm_id"))

	var road_mesh_count := _count_road_mesh_instances(streamer)
	print("STREAM_E2E loaded_road_mesh_count=", road_mesh_count)

	var spawn_position: Vector3 = player.global_position
	var normal_move := await _measure_move(player, spawn_position)
	print("STREAM_E2E normal_move=", normal_move)

	var moved_keys := initial_keys
	if manifest_count > max_loaded:
		var target := _choose_far_manifest_entry(manifest, initial_keys)
		if target.is_empty():
			_fail("missing_far_manifest_target")
			return
		var bounds: Array = target.get("bounds_godot", [])
		player.global_position = Vector3(
			(float(bounds[0]) + float(bounds[2])) * 0.5,
			spawn_position.y + 0.5,
			(float(bounds[1]) + float(bounds[3])) * 0.5
		)
		player.velocity = Vector3.ZERO
		for i in range(24):
			await process_frame
			await physics_frame
		loaded_chunks = streamer.get("loaded_chunks")
		moved_keys = _sorted_keys(loaded_chunks)
		print("STREAM_E2E moved_target_coord=", target.get("coord", []))
		print("STREAM_E2E loaded_after_move=", loaded_chunks.size())
		print("STREAM_E2E moved_keys=", moved_keys)

	var failed := false
	if manifest_count <= 0:
		push_error("STREAM_E2E manifest has no chunks")
		failed = true
	if loaded_initial <= 0:
		push_error("STREAM_E2E streamer loaded no chunks at startup")
		failed = true
	if loaded_initial > max_loaded:
		push_error("STREAM_E2E startup loaded more chunks than stream radius allows")
		failed = true
	if manifest_count > max_loaded and loaded_initial >= manifest_count:
		push_error("STREAM_E2E streamer appears to load the full world")
		failed = true
	if manifest_count > max_loaded and moved_keys == initial_keys:
		push_error("STREAM_E2E loaded chunk set did not change after teleport")
		failed = true
	if not found_meta:
		push_error("STREAM_E2E no loaded generated node exposes OSM metadata")
		failed = true
	if road_mesh_count <= 0:
		push_error("STREAM_E2E no road mesh loaded through chunks")
		failed = true
	if normal_move < MIN_NORMAL_MOVE:
		push_error("STREAM_E2E player did not move in normal mode")
		failed = true

	quit(1 if failed else 0)

func _choose_far_manifest_entry(manifest: Dictionary, initial_keys: Array) -> Dictionary:
	var initial := {}
	for key in initial_keys:
		initial[str(key)] = true
	var chunks: Array = manifest.get("chunks", [])
	for i in range(chunks.size() - 1, -1, -1):
		var entry: Dictionary = chunks[i]
		var coord: Array = entry.get("coord", [])
		if coord.size() < 2:
			continue
		var key := str(int(coord[0])) + ":" + str(int(coord[1]))
		if not initial.has(key):
			return entry
	return {}

func _sorted_keys(dict: Dictionary) -> Array:
	var keys := dict.keys()
	keys.sort()
	return keys

func _find_first_node_with_osm_meta(node: Node) -> Node:
	if node.has_meta("osm_id") and node.has_meta("osm_kind"):
		return node
	for child: Node in node.get_children():
		var found := _find_first_node_with_osm_meta(child)
		if found != null:
			return found
	return null

func _count_road_mesh_instances(node: Node) -> int:
	var count := 0
	if node is MeshInstance3D:
		var material_name := ""
		var mesh_node := node as MeshInstance3D
		var material: Material = mesh_node.get_surface_override_material(0)
		if material != null:
			material_name = str(material.resource_path)
		if str(node.name).begins_with("Highway") or material_name.contains("road_"):
			count += 1
	for child: Node in node.get_children():
		count += _count_road_mesh_instances(child)
	return count

func _wait_for_initial_stream_content(streamer: Node) -> void:
	for i in range(420):
		if _find_first_node_with_osm_meta(streamer) != null and _count_road_mesh_instances(streamer) > 0:
			return
		await process_frame
		await physics_frame

func _measure_move(player: CharacterBody3D, spawn_position: Vector3) -> float:
	player.global_position = spawn_position
	player.velocity = Vector3.ZERO
	for i in range(4):
		await physics_frame
	var start: Vector3 = player.global_position
	_press_key(KEY_W)
	for i in range(45):
		await physics_frame
	_release_key(KEY_W)
	await physics_frame
	var finish: Vector3 = player.global_position
	return Vector2(finish.x - start.x, finish.z - start.z).length()

func _press_key(key: Key) -> void:
	var event := InputEventKey.new()
	event.keycode = key
	event.physical_keycode = key
	event.pressed = true
	Input.parse_input_event(event)

func _release_key(key: Key) -> void:
	var event := InputEventKey.new()
	event.keycode = key
	event.physical_keycode = key
	event.pressed = false
	Input.parse_input_event(event)

func _fail(reason: String) -> void:
	push_error("STREAM_E2E " + reason)
	quit(1)
