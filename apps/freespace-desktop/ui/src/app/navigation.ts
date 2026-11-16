import { useShellStore } from '../stores/shellStore';

/** Opens the tree screen and reveals the given node once the tree loads. */
export function openInTree(nodeId: number): void {
  const shell = useShellStore.getState();
  shell.setPendingNode(nodeId);
  shell.setScreen('tree');
}
