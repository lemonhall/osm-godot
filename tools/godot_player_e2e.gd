extends SceneTree

const MIN_NORMAL_MOVE := 8.0
const MIN_NOCLIP_MOVE := 0.5
const MIN_MOUSE_YAW := 0.001
const MIN_ROAD_MESHES := 1
const MIN_ROAD_WORLD_SPAN := 200.0

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

	for i in range(8):
		await process_frame
		await physics_frame

	var player: CharacterBody3D = scene.get_node_or_null("Player")
	if player == null:
		_fail("missing_player")
		return
	var camera: Camera3D = player.get_node_or_null("Camera3D")
	if camera == null:
		_fail("missing_camera")
		return

	print("E2E player_initial=", player.global_position)
	print("E2E mouse_mode_initial=", Input.get_mouse_mode())
	print("E2E camera_current=", camera.current)
	print("E2E collision_disabled_initial=", player.get_node("CollisionShape3D").disabled)
	print("E2E floor_initial=", player.is_on_floor())
	var spawn_position: Vector3 = player.global_position
	var roads: Node = scene.get_node_or_null("Roads")
	var road_mesh_count := 0
	var road_world_span := 0.0
	if roads != null:
		road_mesh_count = _count_road_mesh_instances(roads)
		var road_bounds: Dictionary = {
			"min_x": INF,
			"max_x": -INF,
			"min_z": INF,
			"max_z": -INF,
		}
		_accumulate_road_mesh_bounds(roads, road_bounds)
		road_world_span = max(
			float(road_bounds["max_x"]) - float(road_bounds["min_x"]),
			float(road_bounds["max_z"]) - float(road_bounds["min_z"])
		)
	print("E2E roads_node_exists=", roads != null)
	print("E2E road_mesh_count=", road_mesh_count)
	print("E2E road_world_span=", road_world_span)
	var osm_meta_node: Node = _find_first_node_with_osm_meta(scene)
	print("E2E osm_meta_node=", "null" if osm_meta_node == null else osm_meta_node.name)
	if osm_meta_node != null:
		print("E2E osm_meta_kind=", osm_meta_node.get_meta("osm_kind"))
		print("E2E osm_meta_id=", osm_meta_node.get_meta("osm_id"))

	var yaw_before: float = player.rotation.y
	var pitch_before: float = camera.rotation.x
	var motion := InputEventMouseMotion.new()
	motion.relative = Vector2(320.0, -80.0)
	Input.parse_input_event(motion)
	await process_frame
	await physics_frame
	var mouse_yaw_delta: float = abs(player.rotation.y - yaw_before)
	var mouse_pitch_delta: float = abs(camera.rotation.x - pitch_before)
	print("E2E yaw_delta=", mouse_yaw_delta)
	print("E2E pitch_delta=", mouse_pitch_delta)

	var normal_move: float = await _measure_move(player, "normal_all_collision", spawn_position)
	_set_named_collision_shapes(scene, "highway", true)
	var no_road_move: float = await _measure_move(player, "normal_without_highway_collision", spawn_position)
	_set_named_collision_shapes(scene, "highway", false)
	_set_named_collision_shapes(scene, "terrain", true)
	var no_terrain_move: float = await _measure_move(player, "normal_without_terrain_collision", spawn_position)
	_set_named_collision_shapes(scene, "highway", true)
	var no_walkable_move: float = await _measure_move(player, "normal_without_highway_or_terrain_collision", spawn_position)
	_set_named_collision_shapes(scene, "highway", false)
	_set_named_collision_shapes(scene, "terrain", false)

	_press_key(KEY_V)
	await process_frame
	_release_key(KEY_V)
	await process_frame

	var noclip_move: float = await _measure_move(player, "noclip", spawn_position, 30)
	print("E2E collision_disabled_after_v=", player.get_node("CollisionShape3D").disabled)

	var failed := false
	if mouse_yaw_delta < MIN_MOUSE_YAW:
		push_error("E2E mouse did not rotate player")
		failed = true
	if normal_move < MIN_NORMAL_MOVE:
		push_error("E2E normal mode did not move player")
		failed = true
	if noclip_move < MIN_NOCLIP_MOVE:
		push_error("E2E noclip mode did not move player")
		failed = true
	if road_mesh_count < MIN_ROAD_MESHES:
		push_error("E2E roads scene did not load visible road meshes")
		failed = true
	if road_world_span < MIN_ROAD_WORLD_SPAN:
		push_error("E2E roads scene meshes are not spread across world coordinates")
		failed = true
	if osm_meta_node == null:
		push_error("E2E no generated road/building node exposes OSM metadata")
		failed = true

	quit(1 if failed else 0)

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

func _accumulate_road_mesh_bounds(node: Node, bounds: Dictionary) -> void:
	if node is MeshInstance3D:
		var mesh_node := node as MeshInstance3D
		var material_name := ""
		var material: Material = mesh_node.get_surface_override_material(0)
		if material != null:
			material_name = str(material.resource_path)
		if str(node.name).begins_with("Highway") or material_name.contains("road_"):
			var origin: Vector3 = mesh_node.global_transform.origin
			bounds["min_x"] = min(float(bounds["min_x"]), origin.x)
			bounds["max_x"] = max(float(bounds["max_x"]), origin.x)
			bounds["min_z"] = min(float(bounds["min_z"]), origin.z)
			bounds["max_z"] = max(float(bounds["max_z"]), origin.z)
	for child: Node in node.get_children():
		_accumulate_road_mesh_bounds(child, bounds)

func _measure_move(player: CharacterBody3D, label: String, spawn_position: Vector3, frames := 90) -> float:
	player.global_position = spawn_position
	player.velocity = Vector3.ZERO
	for i in range(4):
		await physics_frame
	var start: Vector3 = player.global_position
	_press_key(KEY_W)
	for i in range(frames):
		await physics_frame
	_release_key(KEY_W)
	await physics_frame
	var finish: Vector3 = player.global_position
	var xz_move := Vector2(finish.x - start.x, finish.z - start.z).length()
	print("E2E ", label, "_start=", start)
	print("E2E ", label, "_end=", finish)
	print("E2E ", label, "_xz_move=", xz_move)
	print("E2E ", label, "_floor=", player.is_on_floor())
	print("E2E ", label, "_slide_count=", player.get_slide_collision_count())
	for i in range(player.get_slide_collision_count()):
		var collision := player.get_slide_collision(i)
		print("E2E ", label, "_slide_", i, "_normal=", collision.get_normal(), " collider=", collision.get_collider().name)
	return xz_move

func _set_named_collision_shapes(root: Node, token: String, disabled: bool) -> void:
	var count := _set_named_collision_shapes_recursive(root, token, disabled)
	print("E2E collision_shapes_", token, "_disabled_", disabled, "=", count)

func _set_named_collision_shapes_recursive(node: Node, token: String, disabled: bool) -> int:
	var count := 0
	var lower_name := node.name.to_lower()
	if node is CollisionShape3D and node.get_parent() != null and node.get_parent().name.to_lower().contains(token):
		node.disabled = disabled
		count += 1
	for child in node.get_children():
		count += _set_named_collision_shapes_recursive(child, token, disabled)
	return count

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
	push_error("E2E " + reason)
	quit(1)
