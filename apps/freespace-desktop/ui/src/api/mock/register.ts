import type { CommandName } from '../transport';

/** Handler registered in the in-memory transport. */
export type MockHandler = (args: Record<string, unknown>) => unknown;

/** Registration callback handed to a per-story mock plugin. */
export type MockRegister = (command: CommandName, handler: MockHandler) => void;
