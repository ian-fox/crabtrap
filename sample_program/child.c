#include <stdio.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/wait.h>

void make_child(int i) {
    if (i == 0) {
        return;
    }

    pid_t p = fork();
    if (p < 0) {
        perror("fork failed");
        exit(1);
    }

    if (p == 0) {
        make_child(i - 1);
        sleep(i);
        switch (i) {
            case 3: printf("Child %d calling static...\n", i);
                if (execv("/usr/local/bin/static", NULL) == -1) {
                    perror("execv failed");
                    exit(1);
                }
            case 2: printf("Child %d calling dynamic...\n", i);
                if (execv("/usr/local/bin/dynamic", NULL) == -1) {
                    perror("execv failed");
                    exit(1);
                }
            case 1: printf("Child %d calling all-in-one...\n", i);
                if (execv("/usr/local/bin/all-in-one", NULL) == -1) {
                    perror("execv failed");
                    exit(1);
                }
        }
    } else {
        waitpid(p, NULL, 0);
        printf("Goodbye from parent %d!\n", i);
    }
}

int main() {
    make_child(3);
}