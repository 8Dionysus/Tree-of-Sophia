import type { NodeDisplayData, RenderParams } from "sigma/types";
import { NodeProgram, type ProgramInfo } from "sigma/rendering";
import { floatColor } from "sigma/utils";

const UNIFORMS = ["u_sizeRatio", "u_pixelRatio", "u_matrix"] as const;
type StarUniform = (typeof UNIFORMS)[number];

const VERTEX_SHADER_SOURCE = /* glsl */ `
attribute vec4 a_id;
attribute vec4 a_color;
attribute vec2 a_position;
attribute float a_size;

uniform float u_sizeRatio;
uniform float u_pixelRatio;
uniform mat3 u_matrix;

varying vec4 v_color;

const float bias = 255.0 / 254.0;

void main() {
  gl_Position = vec4((u_matrix * vec3(a_position, 1)).xy, 0, 1);
  gl_PointSize = max(4.0, a_size / u_sizeRatio * u_pixelRatio * 2.0);

  #ifdef PICKING_MODE
  v_color = a_id;
  #else
  v_color = a_color;
  #endif

  v_color.a *= bias;
}
`;

const FRAGMENT_SHADER_SOURCE = /* glsl */ `
precision mediump float;

varying vec4 v_color;

const vec4 transparent = vec4(0.0, 0.0, 0.0, 0.0);

void main(void) {
  vec2 p = abs(gl_PointCoord - vec2(0.5)) * 2.0;
  float longVertical = p.x * 4.2 + p.y;
  float longHorizontal = p.x + p.y * 4.2;
  float coreDiamond = (p.x + p.y) * 1.72;
  float starDistance = min(coreDiamond, min(longVertical, longHorizontal));

  #ifdef PICKING_MODE
  if (starDistance <= 1.18)
    gl_FragColor = v_color;
  else
    gl_FragColor = transparent;
  #else
  float solid = 1.0 - smoothstep(0.74, 1.0, starDistance);
  float radiance = (1.0 - smoothstep(0.96, 1.48, starDistance)) * 0.26;
  float alpha = max(solid, radiance);
  gl_FragColor = mix(transparent, v_color, alpha);
  #endif
}
`;

/**
 * A compact four-point star for the public atlas. It keeps Sigma's fast,
 * single-point WebGL path while replacing the generic bubble vocabulary.
 */
export default class StarNodeProgram extends NodeProgram<StarUniform> {
  getDefinition() {
    return {
      VERTICES: 1,
      VERTEX_SHADER_SOURCE,
      FRAGMENT_SHADER_SOURCE,
      METHOD: WebGLRenderingContext.POINTS,
      UNIFORMS,
      ATTRIBUTES: [
        { name: "a_position", size: 2, type: WebGLRenderingContext.FLOAT },
        { name: "a_size", size: 1, type: WebGLRenderingContext.FLOAT },
        { name: "a_color", size: 4, type: WebGLRenderingContext.UNSIGNED_BYTE, normalized: true },
        { name: "a_id", size: 4, type: WebGLRenderingContext.UNSIGNED_BYTE, normalized: true },
      ],
    };
  }

  processVisibleItem(nodeIndex: number, startIndex: number, data: NodeDisplayData): void {
    const array = this.array;
    array[startIndex++] = data.x;
    array[startIndex++] = data.y;
    array[startIndex++] = data.size;
    array[startIndex++] = floatColor(data.color);
    array[startIndex++] = nodeIndex;
  }

  setUniforms(params: RenderParams, { gl, uniformLocations }: ProgramInfo<StarUniform>): void {
    gl.uniform1f(uniformLocations.u_pixelRatio, params.pixelRatio);
    gl.uniform1f(uniformLocations.u_sizeRatio, params.sizeRatio);
    gl.uniformMatrix3fv(uniformLocations.u_matrix, false, params.matrix);
  }
}

