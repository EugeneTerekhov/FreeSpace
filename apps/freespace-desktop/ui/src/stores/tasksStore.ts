import { create } from 'zustand';

import type { Task, TaskProgress } from '../api/dto';

interface TasksState {
  tasks: Task[];
  setTasks: (tasks: Task[]) => void;
  upsert: (task: Task) => void;
  applyProgress: (taskId: string, progress: TaskProgress) => void;
  updateState: (taskId: string, state: Task['state'], finished: boolean) => void;
  clearFinished: () => void;
}

/** Background tasks shown in the shell status bar. */
export const useTasksStore = create<TasksState>()((set) => ({
  tasks: [],

  setTasks(tasks) {
    set({ tasks });
  },

  upsert(task) {
    set((state) => {
      const index = state.tasks.findIndex((item) => item.task_id === task.task_id);
      if (index === -1) {
        return { tasks: [...state.tasks, task] };
      }

      const tasks = [...state.tasks];
      tasks[index] = task;

      return { tasks };
    });
  },

  applyProgress(taskId, progress) {
    set((state) => ({
      tasks: state.tasks.map((task) =>
        task.task_id === taskId
          ? { ...task, state: 'running', progress }
          : task,
      ),
    }));
  },

  updateState(taskId, taskState, finished) {
    set((state) => ({
      tasks: state.tasks.map((task) =>
        task.task_id === taskId
          ? {
              ...task,
              state: taskState,
              finished_at: finished ? Date.now() : task.finished_at,
              cancelable: finished ? false : task.cancelable,
            }
          : task,
      ),
    }));
  },

  clearFinished() {
    set((state) => ({
      tasks: state.tasks.filter(
        (task) =>
          task.state === 'running' ||
          task.state === 'cancelling' ||
          task.state === 'queued',
      ),
    }));
  },
}));
