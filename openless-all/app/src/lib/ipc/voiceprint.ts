// Voiceprint IPC bindings for ZeroType.
import { invokeOrMock } from "./shared"

export interface SpeakerProfile {
  id: string
  name: string
  createdAt: string
  matchCount: number
}

export interface SpeakerEnrollRequest {
  name: string
}

export interface SpeakerMatchResult {
  speakerId: string
  speakerName: string
  confidence: number
  isMatch: boolean
}

export function voiceprintEnroll(
  req: SpeakerEnrollRequest,
  pcmBase64: string,
): Promise<SpeakerProfile> {
  return invokeOrMock(
    "voiceprint_enroll",
    { req, pcmBase64 },
    () => ({
      id: `speaker-${Date.now()}`,
      name: req.name,
      createdAt: new Date().toISOString(),
      matchCount: 0,
    }),
  )
}

export function voiceprintIdentify(
  pcmBase64: string,
): Promise<SpeakerMatchResult | null> {
  return invokeOrMock(
    "voiceprint_identify",
    { pcmBase64 },
    () => null,
  )
}

export function voiceprintList(): Promise<SpeakerProfile[]> {
  return invokeOrMock("voiceprint_list", undefined, () => [
    {
      id: "speaker-1",
      name: "张三",
      createdAt: new Date().toISOString(),
      matchCount: 42,
    },
  ])
}

export function voiceprintRemove(id: string): Promise<void> {
  return invokeOrMock("voiceprint_remove", { id }, () => undefined)
}
