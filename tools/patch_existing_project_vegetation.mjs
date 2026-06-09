#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const DEFAULT_BBOX = "30.67,120.85,31.88,122.12";
const DEFAULT_CHUNK_SIZE = 512;
const DEFAULT_GODOT_SCALE = 0.5;
const DEFAULT_WORLD_SCALE = 1.0;
const DEFAULT_MAX_PLANTS_PER_CHUNK = 96;
const RADIUS_M = 6371000.0;

function main() {
  const args = parseArgs(process.argv.slice(2));
  const projectDir = required(args.project, "--project");
  const tileCacheDir = required(args["tile-cache"], "--tile-cache");
  const bbox = parseBbox(args.bbox ?? DEFAULT_BBOX);
  const chunkSize = intArg(args["chunk-size"], DEFAULT_CHUNK_SIZE);
  const godotScale = floatArg(args["godot-scale"], DEFAULT_GODOT_SCALE);
  const worldScale = floatArg(args.scale, DEFAULT_WORLD_SCALE);
  const maxPlantsPerChunk = intArg(
    args["max-plants-per-chunk"],
    DEFAULT_MAX_PLANTS_PER_CHUNK,
  );

  const meshDataDir = path.join(projectDir, "mesh_data");
  const scenesDir = path.join(projectDir, "scenes");
  const manifestPath = path.join(projectDir, "world_manifest.json");
  requireDir(meshDataDir);
  requireDir(scenesDir);
  requireFile(manifestPath);

  const transform = makeTransform(bbox, worldScale);
  const manifest = readJson(manifestPath);
  const manifestByCoord = new Map();
  for (const chunk of manifest.chunks ?? []) {
    manifestByCoord.set(coordKey(chunk.coord[0], chunk.coord[1]), chunk);
  }

  const patch = buildVegetationPatch({
    tileCacheDir,
    bbox,
    transform,
    chunkSize,
    godotScale,
    maxPlantsPerChunk,
  });

  applyPatch({
    projectDir,
    meshDataDir,
    scenesDir,
    manifestPath,
    manifest,
    manifestByCoord,
    patch,
    chunkSize,
    godotScale,
  });

  const reportPath = path.join(projectDir, "vegetation_patch_report.json");
  fs.writeFileSync(
    reportPath,
    JSON.stringify(
      {
        project: projectDir,
        tile_cache: tileCacheDir,
        bbox,
        chunk_size: chunkSize,
        godot_scale: godotScale,
        world_scale: worldScale,
        max_plants_per_chunk: maxPlantsPerChunk,
        ...patch.stats,
      },
      null,
      2,
    ),
    "utf8",
  );

  console.log(
    [
      "VEGETATION_PATCH",
      `chunks=${patch.byChunk.size}`,
      `areas=${patch.stats.vegetation_areas}`,
      `tree_nodes=${patch.stats.tree_nodes}`,
      `ground=${patch.stats.ground_elements}`,
      `trunks=${patch.stats.trunk_elements}`,
      `trees=${patch.stats.tree_elements}`,
      `conifers=${patch.stats.conifer_elements}`,
      `shrubs=${patch.stats.shrub_elements}`,
      `skipped_plants_by_chunk_cap=${patch.stats.skipped_plants_by_chunk_cap}`,
      `report=${reportPath}`,
    ].join(" "),
  );
}

function buildVegetationPatch({
  tileCacheDir,
  bbox,
  transform,
  chunkSize,
  godotScale,
  maxPlantsPerChunk,
}) {
  const byChunk = new Map();
  const plantCountByChunk = new Map();
  const seenWays = new Set();
  const seenTreeNodes = new Set();
  const stats = {
    tile_files: 0,
    vegetation_areas: 0,
    tree_nodes: 0,
    ground_elements: 0,
    trunk_elements: 0,
    tree_elements: 0,
    conifer_elements: 0,
    shrub_elements: 0,
    skipped_plants_by_chunk_cap: 0,
    missing_way_nodes: 0,
    first_position: null,
  };

  const files = fs
    .readdirSync(tileCacheDir)
    .filter((name) => name.endsWith(".json"))
    .sort();

  for (const fileName of files) {
    stats.tile_files += 1;
    const tilePath = path.join(tileCacheDir, fileName);
    const data = readJson(tilePath);
    const nodes = new Map();
    const ways = [];

    for (const element of data.elements ?? []) {
      if (element.type === "node" && isFiniteNumber(element.lat) && isFiniteNumber(element.lon)) {
        nodes.set(element.id, {
          id: element.id,
          lat: element.lat,
          lon: element.lon,
          tags: element.tags ?? {},
        });
      } else if (element.type === "way") {
        ways.push(element);
      }
    }

    for (const node of nodes.values()) {
      if (node.tags.natural !== "tree" || seenTreeNodes.has(node.id)) {
        continue;
      }
      seenTreeNodes.add(node.id);
      if (!containsLatLon(bbox, node.lat, node.lon)) {
        continue;
      }
      const world = transformPoint(transform, node.lat, node.lon);
      if (!insideWorld(transform, world)) {
        continue;
      }
      const added = addPlant({
        byChunk,
        plantCountByChunk,
        maxPlantsPerChunk,
        stats,
        chunkSize,
        godotScale,
        seed: node.id,
        profile: profileForId(node.id),
        worldX: world.x,
        worldZ: world.z,
      });
      if (added) {
        stats.tree_nodes += 1;
      }
    }

    for (const way of ways) {
      if (seenWays.has(way.id)) {
        continue;
      }
      const kind = classifyVegetation(way.tags ?? {});
      if (!kind || !Array.isArray(way.nodes) || way.nodes.length < 4) {
        continue;
      }
      seenWays.add(way.id);

      const first = way.nodes[0];
      const last = way.nodes[way.nodes.length - 1];
      if (first !== last) {
        continue;
      }

      const footprint = [];
      let missing = false;
      for (const nodeId of way.nodes) {
        const node = nodes.get(nodeId);
        if (!node) {
          missing = true;
          break;
        }
        const point = transformPoint(transform, node.lat, node.lon);
        footprint.push([point.x, point.z]);
      }
      if (missing) {
        stats.missing_way_nodes += 1;
        continue;
      }

      if (!footprint.some((point) => insideWorld(transform, { x: point[0], z: point[1] }))) {
        continue;
      }
      addVegetationArea({
        byChunk,
        plantCountByChunk,
        maxPlantsPerChunk,
        stats,
        chunkSize,
        godotScale,
        osmId: way.id,
        tags: way.tags ?? {},
        kind,
        footprint,
      });
    }
  }

  return { byChunk, stats };
}

function addVegetationArea({
  byChunk,
  plantCountByChunk,
  maxPlantsPerChunk,
  stats,
  chunkSize,
  godotScale,
  osmId,
  tags,
  kind,
  footprint,
}) {
  const [cx, cz] = centroid(footprint);
  const worldX = Math.round(cx);
  const worldZ = Math.round(cz);
  const coord = chunkFor(worldX, worldZ, chunkSize);
  if (!coord) {
    return;
  }

  const polygon = footprint.map(([x, z]) => [
    (x - cx) * godotScale,
    -(z - cz) * godotScale,
  ]);
  const metadata = osmMetadata(osmId, "vegetation", tags);
  metadata.vegetation_kind = kind.name;
  addElement(
    byChunk,
    coord,
    meshElement({
      name: `VegetationGround_${osmId}`,
      material: kind.groundMaterial,
      mesh: makeRoofFlat(polygon, 0.03),
      transform: translationTransform(
        (worldX - coord[0] * chunkSize) * godotScale,
        0.0,
        -((worldZ - coord[1] * chunkSize) * godotScale),
      ),
      metadata,
    }),
  );
  if (stats.first_position == null) {
    stats.first_position = [worldX * godotScale, 0.0, -(worldZ * godotScale)];
  }
  stats.vegetation_areas += 1;
  stats.ground_elements += 1;

  const points = scatterPoints(osmId, kind, footprint);
  for (let i = 0; i < points.length; i += 1) {
    const point = points[i];
    addPlant({
      byChunk,
      plantCountByChunk,
      maxPlantsPerChunk,
      stats,
      chunkSize,
      godotScale,
        seed: BigInt(osmId) * 10000n + BigInt(i),
      profile: point.profile,
      worldX: point.x,
      worldZ: point.z,
    });
  }
}

function addPlant({
  byChunk,
  plantCountByChunk,
  maxPlantsPerChunk,
  stats,
  chunkSize,
  godotScale,
  seed,
  profile,
  worldX,
  worldZ,
}) {
  const coord = chunkFor(worldX, worldZ, chunkSize);
  if (!coord) {
    return false;
  }
  const key = coordKey(coord[0], coord[1]);
  const currentCount = plantCountByChunk.get(key) ?? 0;
  if (currentCount >= maxPlantsPerChunk) {
    stats.skipped_plants_by_chunk_cap += 1;
    return false;
  }
  plantCountByChunk.set(key, currentCount + 1);

  const seedValue = BigInt(seed);
  const rotation = unitHash(seedValue ^ 0x8f3d2a91n) * Math.PI * 2.0;
  const scale = 0.85 + unitHash(seedValue ^ 0x51abc309n) * 0.45;
  const localX = (worldX - coord[0] * chunkSize) * godotScale;
  const localZ = -((worldZ - coord[1] * chunkSize) * godotScale);

  if (profile === "broadleaf") {
    const trunkHeight = 2.1 * scale;
    addElement(
      byChunk,
      coord,
      meshElement({
        name: `VegetationTrunk_${seed}`,
        material: "tree_trunk",
        mesh: makeCylinder(0.22 * scale, trunkHeight, 7),
        transform: transformFromPosRot(localX, 0.0, localZ, rotation),
        metadata: {},
      }),
    );
    addElement(
      byChunk,
      coord,
      meshElement({
        name: `VegetationTree_${seed}`,
        material: "tree_leaves",
        mesh: offsetMesh(makeCylinder(1.25 * scale, 1.45 * scale, 10), 0, trunkHeight, 0),
        transform: transformFromPosRot(localX, 0.0, localZ, rotation),
        metadata: {},
      }),
    );
    stats.trunk_elements += 1;
    stats.tree_elements += 1;
  } else if (profile === "conifer") {
    const trunkHeight = 1.25 * scale;
    addElement(
      byChunk,
      coord,
      meshElement({
        name: `VegetationTrunk_${seed}`,
        material: "tree_trunk",
        mesh: makeCylinder(0.18 * scale, trunkHeight, 6),
        transform: transformFromPosRot(localX, 0.0, localZ, rotation),
        metadata: {},
      }),
    );
    addElement(
      byChunk,
      coord,
      meshElement({
        name: `VegetationConifer_${seed}`,
        material: "tree_leaves",
        mesh: offsetMesh(makeCone(1.15 * scale, 3.6 * scale, 9), 0, trunkHeight * 0.45, 0),
        transform: transformFromPosRot(localX, 0.0, localZ, rotation),
        metadata: {},
      }),
    );
    stats.trunk_elements += 1;
    stats.conifer_elements += 1;
  } else {
    addElement(
      byChunk,
      coord,
      meshElement({
        name: `VegetationShrub_${seed}`,
        material: "tree_leaves",
        mesh: offsetMesh(makeCylinder(0.95 * scale, 0.85 * scale, 8), 0, 0.08, 0),
        transform: transformFromPosRot(localX, 0.0, localZ, rotation),
        metadata: {},
      }),
    );
    stats.shrub_elements += 1;
  }

  return true;
}

function applyPatch({
  projectDir,
  meshDataDir,
  scenesDir,
  manifestPath,
  manifest,
  manifestByCoord,
  patch,
  chunkSize,
  godotScale,
}) {
  for (const [key, newElements] of patch.byChunk) {
    const [cx, cz] = key.split(",").map((value) => Number.parseInt(value, 10));
    const meshPath = path.join(meshDataDir, `Chunk_${cx}_${cz}.json`);
    let payload = { elements: [] };
    if (fs.existsSync(meshPath)) {
      payload = readJson(meshPath);
    } else {
      writeChunkScene(scenesDir, cx, cz);
    }

    payload.elements = (payload.elements ?? []).filter(
      (element) => !String(element.name ?? "").startsWith("Vegetation"),
    );
    payload.elements.push(...newElements);
    fs.writeFileSync(meshPath, JSON.stringify(payload), "utf8");

    let manifestChunk = manifestByCoord.get(key);
    if (!manifestChunk) {
      manifestChunk = makeManifestChunk(cx, cz, chunkSize, godotScale);
      manifest.chunks.push(manifestChunk);
      manifestByCoord.set(key, manifestChunk);
    }
    manifestChunk.element_count = payload.elements.length;
  }

  manifest.chunks.sort((a, b) => {
    const dx = a.coord[0] - b.coord[0];
    return dx !== 0 ? dx : a.coord[1] - b.coord[1];
  });
  manifest.chunk_size_blocks = chunkSize;
  manifest.godot_scale = godotScale;
  manifest.vegetation_patch = {
    tool: "tools/patch_existing_project_vegetation.mjs",
    generated_at: new Date().toISOString(),
    chunks: patch.byChunk.size,
    vegetation_areas: patch.stats.vegetation_areas,
    tree_nodes: patch.stats.tree_nodes,
  };
  fs.writeFileSync(manifestPath, JSON.stringify(manifest), "utf8");

  const readmePath = path.join(projectDir, "VEGETATION_PATCH.md");
  fs.writeFileSync(
    readmePath,
    [
      "# Vegetation Patch",
      "",
      "This generated project was patched in-place with OSM vegetation mesh data.",
      "",
      `- Tool: tools/patch_existing_project_vegetation.mjs`,
      `- Patched chunks: ${patch.byChunk.size}`,
      `- Vegetation areas: ${patch.stats.vegetation_areas}`,
      `- Tree nodes: ${patch.stats.tree_nodes}`,
      `- Generated elements: ${patch.stats.ground_elements + patch.stats.trunk_elements + patch.stats.tree_elements + patch.stats.conifer_elements + patch.stats.shrub_elements}`,
      "",
    ].join("\n"),
    "utf8",
  );
}

function makeManifestChunk(cx, cz, chunkSize, godotScale) {
  const minX = cx * chunkSize;
  const minZ = cz * chunkSize;
  const maxX = minX + chunkSize - 1;
  const maxZ = minZ + chunkSize - 1;
  return {
    bounds_godot: [
      minX * godotScale,
      -(maxZ * godotScale),
      maxX * godotScale,
      -(minZ * godotScale),
    ],
    coord: [cx, cz],
    element_count: 0,
    mesh_data_path: `res://mesh_data/Chunk_${cx}_${cz}.json`,
    origin: [minX * godotScale, -(minZ * godotScale)],
    road_count: 0,
    scene_path: `res://scenes/Chunk_${cx}_${cz}.tscn`,
    world_bounds_blocks: [minX, minZ, maxX, maxZ],
  };
}

function writeChunkScene(scenesDir, cx, cz) {
  const rootName = `Chunk_${cx}_${cz}`;
  const scenePath = path.join(scenesDir, `${rootName}.tscn`);
  const uid = chunkUid(cx, cz);
  const text = [
    `[gd_scene load_steps=2 format=3 uid="uid://${uid}"]`,
    "",
    `[ext_resource type="Script" path="res://scripts/chunk_mesh_loader.gd" id="1"]`,
    "",
    `[node name="${rootName}" type="Node3D"]`,
    `script = ExtResource("1")`,
    `mesh_data_path = "res://mesh_data/${rootName}.json"`,
    "",
  ].join("\n");
  fs.writeFileSync(scenePath, text, "utf8");
}

function chunkUid(cx, cz) {
  let h = BigInt(cx >>> 0) * 0x517cc1b7n;
  h = (h + BigInt(cz >>> 0)) * 0x9e3779b9n;
  h &= (1n << 64n) - 1n;
  return `c${h.toString(16).padStart(13, "0").slice(-13)}`;
}

function classifyVegetation(tags) {
  if (tags.landuse === "forest") return kind("woodland", "tree_leaves", 48, 18);
  if (["grass", "meadow", "recreation_ground", "village_green"].includes(tags.landuse)) {
    return kind("grass", "terrain_grass", 72, 8);
  }
  if (tags.natural === "wood") return kind("woodland", "tree_leaves", 48, 18);
  if (["scrub", "heath"].includes(tags.natural)) return kind("scrub", "tree_leaves", 42, 18);
  if (tags.natural === "grassland") return kind("grass", "terrain_grass", 72, 8);
  if (["park", "garden"].includes(tags.leisure)) return kind("park", "terrain_grass", 56, 14);
  return null;
}

function kind(name, groundMaterial, spacing, maxInstances) {
  return {
    name,
    groundMaterial,
    spacing,
    maxInstances,
    profileFor(osmId, sampleIndex) {
      const h = stableHash(BigInt(osmId) * 97n + BigInt(sampleIndex));
      if (name === "woodland") return Number(h % 3n) === 0 ? "broadleaf" : Number(h % 3n) === 1 ? "conifer" : "shrub";
      if (name === "park") return h % 4n === 0n ? "shrub" : "broadleaf";
      if (name === "grass") return "shrub";
      return h % 5n === 0n ? "broadleaf" : "shrub";
    },
  };
}

function scatterPoints(osmId, kindValue, polygon) {
  const [minX, maxX, minZ, maxZ] = bounds(polygon);
  const points = [];
  let sampleIndex = 0n;
  for (
    let x = minX + kindValue.spacing * 0.5;
    x <= maxX && points.length < kindValue.maxInstances;
    x += kindValue.spacing
  ) {
    for (
      let z = minZ + kindValue.spacing * 0.5;
      z <= maxZ && points.length < kindValue.maxInstances;
      z += kindValue.spacing
    ) {
      const jitterX = (unitHash(BigInt(osmId) ^ sampleIndex) - 0.5) * kindValue.spacing * 0.45;
      const jitterZ =
        (unitHash(BigInt(osmId) ^ rotl64(sampleIndex, 17)) - 0.5) * kindValue.spacing * 0.45;
      const px = x + jitterX;
      const pz = z + jitterZ;
      if (pointInPolygon(px, pz, polygon)) {
        points.push({
          x: Math.round(px),
          z: Math.round(pz),
          profile: kindValue.profileFor(osmId, sampleIndex),
        });
      }
      sampleIndex += 1n;
    }
  }
  return points;
}

function pointInPolygon(px, pz, polygon) {
  let inside = false;
  let j = polygon.length - 1;
  for (let i = 0; i < polygon.length; i += 1) {
    const [xi, zi] = polygon[i];
    const [xj, zj] = polygon[j];
    const crosses = zi > pz !== zj > pz;
    if (crosses) {
      const xIntersect = ((xj - xi) * (pz - zi)) / Math.max(Math.abs(zj - zi), 0.0001) + xi;
      if (px < xIntersect) {
        inside = !inside;
      }
    }
    j = i;
  }
  return inside;
}

function makeRoofFlat(polygon, height) {
  const mesh = emptyMesh();
  if (polygon.length < 3) return mesh;
  for (const [x, z] of polygon) {
    mesh.vertices.push(x, height, z);
    mesh.normals.push(0, 1, 0);
    mesh.uvs.push(x * 0.1, z * 0.1);
  }
  for (let i = 1; i < polygon.length - 1; i += 1) {
    mesh.indices.push(0, i, i + 1);
  }
  const base = mesh.vertices.length / 3;
  for (const [x, z] of polygon) {
    mesh.vertices.push(x, height - 0.1, z);
    mesh.normals.push(0, -1, 0);
    mesh.uvs.push(x * 0.1, z * 0.1);
  }
  for (let i = 1; i < polygon.length - 1; i += 1) {
    mesh.indices.push(base, base + i + 1, base + i);
  }
  return mesh;
}

function makeCylinder(radius, height, segments) {
  const mesh = emptyMesh();
  const n = Math.max(3, segments);
  const topCenter = addVertex(mesh, 0, height, 0, 0, 1, 0, 0.5, 0.5);
  const bottomCenter = addVertex(mesh, 0, 0, 0, 0, -1, 0, 0.5, 0.5);
  const top = [];
  const bottom = [];
  for (let i = 0; i < n; i += 1) {
    const a = (i / n) * Math.PI * 2;
    const x = Math.cos(a) * radius;
    const z = Math.sin(a) * radius;
    top.push(addVertex(mesh, x, height, z, 0, 1, 0, i / n, 1));
    bottom.push(addVertex(mesh, x, 0, z, 0, -1, 0, i / n, 0));
  }
  for (let i = 0; i < n; i += 1) {
    const j = (i + 1) % n;
    mesh.indices.push(topCenter, top[i], top[j]);
    mesh.indices.push(bottomCenter, bottom[j], bottom[i]);
    const base = mesh.vertices.length / 3;
    const a = (i / n) * Math.PI * 2;
    const b = (j / n) * Math.PI * 2;
    const nx = Math.cos((a + b) * 0.5);
    const nz = Math.sin((a + b) * 0.5);
    addVertex(mesh, Math.cos(a) * radius, height, Math.sin(a) * radius, nx, 0, nz, 0, 1);
    addVertex(mesh, Math.cos(b) * radius, height, Math.sin(b) * radius, nx, 0, nz, 1, 1);
    addVertex(mesh, Math.cos(b) * radius, 0, Math.sin(b) * radius, nx, 0, nz, 1, 0);
    addVertex(mesh, Math.cos(a) * radius, 0, Math.sin(a) * radius, nx, 0, nz, 0, 0);
    mesh.indices.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }
  return mesh;
}

function makeCone(radius, height, segments) {
  const mesh = emptyMesh();
  const n = Math.max(3, segments);
  const tip = addVertex(mesh, 0, height, 0, 0, 1, 0, 0.5, 1);
  const ring = [];
  for (let i = 0; i < n; i += 1) {
    const a = (i / n) * Math.PI * 2;
    ring.push(addVertex(mesh, Math.cos(a) * radius, 0, Math.sin(a) * radius, 0, -1, 0, i / n, 0));
  }
  const baseCenter = addVertex(mesh, 0, 0, 0, 0, -1, 0, 0.5, 0);
  for (let i = 0; i < n; i += 1) {
    const j = (i + 1) % n;
    mesh.indices.push(tip, ring[i], ring[j]);
    mesh.indices.push(baseCenter, ring[j], ring[i]);
  }
  return mesh;
}

function emptyMesh() {
  return { vertices: [], normals: [], uvs: [], indices: [] };
}

function addVertex(mesh, x, y, z, nx, ny, nz, u, v) {
  const index = mesh.vertices.length / 3;
  mesh.vertices.push(x, y, z);
  mesh.normals.push(nx, ny, nz);
  mesh.uvs.push(u, v);
  return index;
}

function offsetMesh(mesh, x, y, z) {
  for (let i = 0; i < mesh.vertices.length; i += 3) {
    mesh.vertices[i] += x;
    mesh.vertices[i + 1] += y;
    mesh.vertices[i + 2] += z;
  }
  return mesh;
}

function meshElement({ name, material, mesh, transform, metadata }) {
  return {
    indices: mesh.indices,
    material,
    metadata,
    name,
    normals: mesh.normals,
    transform,
    uvs: mesh.uvs,
    vertices: mesh.vertices,
  };
}

function addElement(byChunk, coord, element) {
  const key = coordKey(coord[0], coord[1]);
  if (!byChunk.has(key)) byChunk.set(key, []);
  byChunk.get(key).push(element);
}

function transformFromPosRot(x, y, z, rot) {
  const s = Math.sin(rot);
  const c = Math.cos(rot);
  return [c, 0, -s, 0, 1, 0, s, 0, c, x, y, z];
}

function translationTransform(x, y, z) {
  return [1, 0, 0, 0, 1, 0, 0, 0, 1, x, y, z];
}

function makeTransform(bbox, scale) {
  const scaleFactorZ = Math.floor(latDistance(bbox.minLat, bbox.maxLat)) * scale;
  const scaleFactorX = Math.floor(lonDistance((bbox.minLat + bbox.maxLat) / 2, bbox.minLon, bbox.maxLon)) * scale;
  return {
    ...bbox,
    lenLat: bbox.maxLat - bbox.minLat,
    lenLon: bbox.maxLon - bbox.minLon,
    scaleFactorX,
    scaleFactorZ,
    maxX: Math.trunc(scaleFactorX),
    maxZ: Math.trunc(scaleFactorZ),
  };
}

function transformPoint(t, lat, lon) {
  const relX = (lon - t.minLon) / t.lenLon;
  const relZ = 1.0 - (lat - t.minLat) / t.lenLat;
  return { x: Math.trunc(relX * t.scaleFactorX), z: Math.trunc(relZ * t.scaleFactorZ) };
}

function insideWorld(t, point) {
  return point.x >= 0 && point.x <= t.maxX && point.z >= 0 && point.z <= t.maxZ;
}

function containsLatLon(bbox, lat, lon) {
  return lat >= bbox.minLat && lat <= bbox.maxLat && lon >= bbox.minLon && lon <= bbox.maxLon;
}

function latDistance(lat1, lat2) {
  const dLat = deg2rad(lat2 - lat1);
  const a = Math.sin(dLat / 2) ** 2;
  return RADIUS_M * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

function lonDistance(lat, lon1, lon2) {
  const dLon = deg2rad(lon2 - lon1);
  const latRad = deg2rad(lat);
  const a = Math.cos(latRad) ** 2 * Math.sin(dLon / 2) ** 2;
  return RADIUS_M * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

function deg2rad(value) {
  return (value * Math.PI) / 180;
}

function chunkFor(worldX, worldZ, chunkSize) {
  if (worldX < 0 || worldZ < 0) return null;
  return [Math.floor(worldX / chunkSize), Math.floor(worldZ / chunkSize)];
}

function coordKey(cx, cz) {
  return `${cx},${cz}`;
}

function centroid(points) {
  let sx = 0;
  let sz = 0;
  for (const [x, z] of points) {
    sx += x;
    sz += z;
  }
  return [sx / points.length, sz / points.length];
}

function bounds(points) {
  let minX = Infinity;
  let maxX = -Infinity;
  let minZ = Infinity;
  let maxZ = -Infinity;
  for (const [x, z] of points) {
    minX = Math.min(minX, x);
    maxX = Math.max(maxX, x);
    minZ = Math.min(minZ, z);
    maxZ = Math.max(maxZ, z);
  }
  return [minX, maxX, minZ, maxZ];
}

function profileForId(id) {
  const h = stableHash(BigInt(id));
  const value = Number(h % 3n);
  return value === 0 ? "broadleaf" : value === 1 ? "conifer" : "shrub";
}

function stableHash(value) {
  const mask = (1n << 64n) - 1n;
  let x = (BigInt(value) + 0x9e3779b97f4a7c15n) & mask;
  x = ((x ^ (x >> 30n)) * 0xbf58476d1ce4e5b9n) & mask;
  x = ((x ^ (x >> 27n)) * 0x94d049bb133111ebn) & mask;
  return (x ^ (x >> 31n)) & mask;
}

function unitHash(value) {
  return Number(stableHash(value) >> 40n) / 16777215.0;
}

function rotl64(value, bits) {
  const mask = (1n << 64n) - 1n;
  const x = BigInt(value) & mask;
  return ((x << BigInt(bits)) | (x >> BigInt(64 - bits))) & mask;
}

function osmMetadata(id, kindValue, tags) {
  const keys = [
    "name",
    "name:zh",
    "name:zh-Hans",
    "name:zh-Hant",
    "official_name",
    "official_name:zh",
    "official_name:zh-Hans",
    "official_name:zh-Hant",
    "alt_name",
    "alt_name:zh",
    "alt_name:zh-Hans",
    "alt_name:zh-Hant",
    "old_name",
    "brand:zh",
    "operator:zh",
    "building",
    "building:levels",
    "building:height",
    "height",
    "roof:shape",
    "roof:material",
    "roof:colour",
    "highway",
    "landuse",
    "natural",
    "leisure",
    "amenity",
    "shop",
    "tourism",
  ];
  const metadata = { osm_id: String(id), osm_kind: kindValue };
  for (const key of keys) {
    if (Object.hasOwn(tags, key)) metadata[key] = String(tags[key]);
  }
  for (const [key, value] of Object.entries(tags)) {
    if (key.startsWith("addr:")) metadata[key] = String(value);
  }
  return metadata;
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) continue;
    const eq = token.indexOf("=");
    if (eq >= 0) {
      args[token.slice(2, eq)] = token.slice(eq + 1);
    } else {
      args[token.slice(2)] = argv[i + 1];
      i += 1;
    }
  }
  return args;
}

function parseBbox(raw) {
  const values = raw.split(",").map((value) => Number.parseFloat(value.trim()));
  if (values.length !== 4 || values.some((value) => !Number.isFinite(value))) {
    throw new Error(`Invalid --bbox: ${raw}`);
  }
  return {
    minLat: values[0],
    minLon: values[1],
    maxLat: values[2],
    maxLon: values[3],
  };
}

function intArg(raw, fallback) {
  if (raw == null) return fallback;
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value)) throw new Error(`Invalid integer: ${raw}`);
  return value;
}

function floatArg(raw, fallback) {
  if (raw == null) return fallback;
  const value = Number.parseFloat(raw);
  if (!Number.isFinite(value)) throw new Error(`Invalid float: ${raw}`);
  return value;
}

function required(value, name) {
  if (!value) throw new Error(`Missing ${name}`);
  return value;
}

function requireDir(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    throw new Error(`Missing directory: ${dir}`);
  }
}

function requireFile(file) {
  if (!fs.existsSync(file) || !fs.statSync(file).isFile()) {
    throw new Error(`Missing file: ${file}`);
  }
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function isFiniteNumber(value) {
  return typeof value === "number" && Number.isFinite(value);
}

main();
