#include <libproc.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>

struct process_entry {
  pid_t pid;
  pid_t ppid;
  int tracked;
};

int main(int argc, char **argv) {
  if (argc < 2) {
    return 2;
  }

  int capacity = proc_listallpids(NULL, 0);
  if (capacity <= 0) {
    return 1;
  }

  capacity += 64;
  pid_t *pids = calloc((size_t)capacity, sizeof(pid_t));
  if (pids == NULL) {
    return 1;
  }

  int count = proc_listallpids(pids, capacity * (int)sizeof(pid_t));
  if (count <= 0) {
    free(pids);
    return 1;
  }

  struct process_entry *entries =
      calloc((size_t)count, sizeof(struct process_entry));
  if (entries == NULL) {
    free(pids);
    return 1;
  }

  for (int index = 0; index < count; index += 1) {
    pid_t pid = pids[index];
    struct proc_bsdinfo bsd_info;
    int bsd_size =
        proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &bsd_info, sizeof(bsd_info));
    if (bsd_size != sizeof(bsd_info)) {
      continue;
    }

    entries[index].pid = pid;
    entries[index].ppid = bsd_info.pbi_ppid;
    for (int argument = 1; argument < argc; argument += 1) {
      if (pid == (pid_t)strtol(argv[argument], NULL, 10)) {
        entries[index].tracked = 1;
        break;
      }
    }
  }

  int changed;
  do {
    changed = 0;
    for (int index = 0; index < count; index += 1) {
      if (entries[index].pid == 0 || entries[index].tracked) {
        continue;
      }
      for (int parent = 0; parent < count; parent += 1) {
        if (entries[parent].tracked &&
            entries[index].ppid == entries[parent].pid) {
          entries[index].tracked = 1;
          changed = 1;
          break;
        }
      }
    }
  } while (changed);

  for (int index = 0; index < count; index += 1) {
    if (!entries[index].tracked) {
      continue;
    }

    pid_t pid = entries[index].pid;
    struct rusage_info_v4 usage;
    if (proc_pid_rusage(pid, RUSAGE_INFO_V4, (rusage_info_t *)&usage) != 0) {
      continue;
    }

    char path[PROC_PIDPATHINFO_MAXSIZE] = {0};
    if (proc_pidpath(pid, path, sizeof(path)) <= 0) {
      snprintf(path, sizeof(path), "pid-%d", pid);
    }

    printf("%d\t%d\t%llu\t%llu\t%llu\t%s\n", pid, entries[index].ppid,
           usage.ri_resident_size, usage.ri_phys_footprint,
           usage.ri_lifetime_max_phys_footprint, path);
  }

  free(entries);
  free(pids);
  return 0;
}
