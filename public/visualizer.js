const VERTEX_SHADER = `
  attribute vec2 position;

  void main() {
    gl_Position = vec4(position, 0.0, 1.0);
  }
`;

const FRAGMENT_SHADER = `
  precision mediump float;

  uniform vec2 resolution;
  uniform float time;
  uniform vec4 levels;
  uniform float peak;
  uniform vec3 paletteLow;
  uniform vec3 paletteMid;
  uniform vec3 paletteHigh;

  float waveGlow(vec2 point, float baseline, float frequency, float speed, float amplitude, float width) {
    float envelope = sin(point.x * 3.14159265);
    float primary = sin(point.x * frequency + time * speed);
    float detail = sin(point.x * frequency * 2.7 - time * speed * 1.4) * 0.24;
    float wave = baseline + (primary + detail) * amplitude * envelope;
    float distanceToWave = abs(point.y - wave);
    return smoothstep(width, 0.0, distanceToWave);
  }

  void main() {
    vec2 point = gl_FragCoord.xy / resolution;
    float bass = levels.x;
    float mids = levels.y;
    float treble = levels.z;
    float energy = levels.w;
    float pulse = 0.65 + peak * 0.65;

    float low = waveGlow(point, 0.38, 9.5, 1.35, 0.035 + bass * 0.19, 0.085 + bass * 0.055);
    float middle = waveGlow(point, 0.50, 15.0, -1.05, 0.025 + mids * 0.14, 0.060 + mids * 0.035);
    float high = waveGlow(point, 0.62, 24.0, 1.8, 0.018 + treble * 0.10, 0.042 + treble * 0.025);

    vec3 lowColor = paletteLow * low * (0.30 + bass * 0.95);
    vec3 midColor = paletteMid * middle * (0.24 + mids * 0.82);
    vec3 highColor = paletteHigh * high * (0.20 + treble * 0.75);
    vec3 color = (lowColor + midColor + highColor) * pulse;
    float alpha = clamp((low + middle + high) * (0.10 + energy * 0.34), 0.0, 0.72);

    gl_FragColor = vec4(color, alpha);
  }
`;

const LEVEL_KEYS = ["bass", "mids", "treble", "energy", "peak"];
const DEFAULT_COLORS = {
  bass: "#d46e42",
  mids: "#994033",
  treble: "#572929",
};

function rgbFromHex(value, fallback) {
  const hex = /^#[0-9a-f]{6}$/i.test(value) ? value : fallback;
  return new Float32Array([
    Number.parseInt(hex.slice(1, 3), 16) / 255,
    Number.parseInt(hex.slice(3, 5), 16) / 255,
    Number.parseInt(hex.slice(5, 7), 16) / 255,
  ]);
}

function compileShader(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function createProgram(gl) {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
  if (!vertex || !fragment) return null;

  const program = gl.createProgram();
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    gl.deleteProgram(program);
    return null;
  }
  return program;
}

export function createVisualizer(canvas) {
  const gl = canvas.getContext("webgl", {
    alpha: true,
    antialias: false,
    depth: false,
    powerPreference: "low-power",
    premultipliedAlpha: true,
    preserveDrawingBuffer: false,
    stencil: false,
  });
  const program = gl && createProgram(gl);
  if (!gl || !program) {
    canvas.dataset.renderer = "unavailable";
    return {
      available: false,
      clear() {},
      render() {},
      setColors() {},
    };
  }

  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.useProgram(program);

  const position = gl.getAttribLocation(program, "position");
  gl.enableVertexAttribArray(position);
  gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);

  const uniforms = {
    resolution: gl.getUniformLocation(program, "resolution"),
    time: gl.getUniformLocation(program, "time"),
    levels: gl.getUniformLocation(program, "levels"),
    peak: gl.getUniformLocation(program, "peak"),
    paletteLow: gl.getUniformLocation(program, "paletteLow"),
    paletteMid: gl.getUniformLocation(program, "paletteMid"),
    paletteHigh: gl.getUniformLocation(program, "paletteHigh"),
  };
  const current = { bass: 0, mids: 0, treble: 0, energy: 0, peak: 0 };
  let palette = {
    bass: rgbFromHex(DEFAULT_COLORS.bass, DEFAULT_COLORS.bass),
    mids: rgbFromHex(DEFAULT_COLORS.mids, DEFAULT_COLORS.mids),
    treble: rgbFromHex(DEFAULT_COLORS.treble, DEFAULT_COLORS.treble),
  };
  let contextLost = false;
  canvas.dataset.renderer = "webgl";

  canvas.addEventListener("webglcontextlost", event => {
    event.preventDefault();
    contextLost = true;
  });
  canvas.addEventListener("webglcontextrestored", () => {
    contextLost = false;
  });

  function resize() {
    const rect = canvas.getBoundingClientRect();
    const scale = Math.min(window.devicePixelRatio || 1, 1.5) * 0.7;
    const width = Math.max(1, Math.min(1400, Math.floor(rect.width * scale)));
    const height = Math.max(1, Math.min(900, Math.floor(rect.height * scale)));
    if (canvas.width === width && canvas.height === height) return;
    canvas.width = width;
    canvas.height = height;
    gl.viewport(0, 0, width, height);
  }

  function clear() {
    if (contextLost) return;
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }

  function setColors(colors) {
    palette = {
      bass: rgbFromHex(colors?.bass, DEFAULT_COLORS.bass),
      mids: rgbFromHex(colors?.mids, DEFAULT_COLORS.mids),
      treble: rgbFromHex(colors?.treble, DEFAULT_COLORS.treble),
    };
  }

  function render(timestamp, target) {
    if (contextLost) return;
    resize();
    for (const key of LEVEL_KEYS) {
      const value = Math.max(0, Math.min(1, Number(target[key]) || 0));
      const smoothing = value > current[key] ? 0.42 : 0.12;
      current[key] += (value - current[key]) * smoothing;
    }

    clear();
    gl.useProgram(program);
    gl.uniform2f(uniforms.resolution, canvas.width, canvas.height);
    gl.uniform1f(uniforms.time, timestamp / 1000);
    gl.uniform4f(uniforms.levels, current.bass, current.mids, current.treble, current.energy);
    gl.uniform1f(uniforms.peak, current.peak);
    gl.uniform3fv(uniforms.paletteLow, palette.bass);
    gl.uniform3fv(uniforms.paletteMid, palette.mids);
    gl.uniform3fv(uniforms.paletteHigh, palette.treble);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  return { available: true, clear, render, setColors };
}
