# SOS

## Why make an OS?
1. For fun

    First and foremost for me this is a fun experiment. I intend to use it to deploy some real applications for personal usage, but as long as this is the highest priority I will never recommend another production usage. In the unlikely case that someone else wants to use this, please reach out! My recommendation would likely be to create a fork of the project with different priorities.
1. To learn more about how OSes really work
1. To experiment with design choices

    Modern OSes have a lot of history. They're extensively tested, researched, and optimized, but they're also building on core design decisions that were made many decades ago.

    For instance, Unix was designed for multi-user mainframes connected to from dumb terminals, adapted to support single-user GUI desktops, thin servers and clusters, hosted environments, and phones. It has core file abstractions that are designed for high-latency spinning disks on the current machine, while many of its practical uses are deployed using SSDs, or network shared storage.

    This project is an experiment in what an OS designed and specialized for modern cloud compute environments could look like.

## Inspiration
1. [Cap'n Proto](https://capnproto.org/)
    - Typed service interfaces
    - "Time travel" - reduce number of round-trip times between services by sending a graph of related service calls
1. Facebook's autoscaling and service discovery system
    - [Scaling services with Shard Manager](https://engineering.fb.com/2020/08/24/production-engineering/scaling-services-with-shard-manager/)
    - DNS for service implementations
    - Load balancing and scaling primitives
1. Async/Await
    - [Asynchronous computing @ Facebook](https://engineering.fb.com/2020/08/17/production-engineering/async/)
    - Implicit efficient graph execution for service calls and system calls
1. [LevelDB](https://github.com/google/leveldb) / [RocksDB](http://rocksdb.org/)
    - Typed single-node filesystems with extremely fast search and read
1. Chrome/Android app sandboxing
    - Simplified privacy and security primitives
    - True app sandboxing built in
1. Kubernetes
    - Services as the core abstraction
    - Declarative service deployments configuring distribution, scaling, and resource allocation
1. Docker
    - Simple, composable application definition and sharing
1. [Cockroach](https://github.com/cockroachdb/cockroach)
    - Raft consensus protocol for distributed data with a common and developer-friendly interface

## Core ideas
- Distributed
    - Clusters can operate effectively with very different allocations of resources.
    - Nodes can operate without things that might be considered core to other OSes; for instance, compute nodes might conceivably have no local filesystem.
    - Distributed primitives like raft-consensus shared data, shared block storage, redundancy, etc.
    - Masterless - no node going offline should be more critical than any other
    - The service call as a core primitive
    - Cluster services defined declaratively, and can reference each other via a DNS-like system, eg. `service("time")`
- Extremely light
    - The OS can run on minimal hardware. Kubenetes suggests minimum resources in the gigabytes of RAM, while SOS should support very small nodes, eg. Pi clusters.
- Typed, fast, versioned filesystems
    - Specialized support for the types of "files" actually needed by applications
    - Typed key-value storage, shared raft-consensus data, tables, logs, shared block storage
- Simple sandboxed applications
    - The service is the unit of execution on the system, as opposed to a set of `(user, process)` pairs
    - Applications have a simple definition, which can be composed via declarative docker-like ideas
    - As a strawman, standard executables might be a binary along with a set of key-value pairs as a filesystem, then deployment consists of a copy-on-write copy of the filesystem, the binary, and an optional set of deployment parameters such as service name, replication strategy, etc. All "files" for the app start with the service name as a prefix, making it impossible for the process to access files for any other application. Running the application looks very similar to running a docker container, setting up a stack and event loop for the process and running it.
    - Applications have no knowledge or direct interaction with other applications on the local system, except opaquely through their service call interface
- Async/Await
    - Async/await for all system and service calls
    - Hybrid preemptive / cooperative multitasking
        - Applications have their own stack, which can utilize multiple cores via Send+Sync futures, but internally use cooperative multitasking
        - The event loop for the process is trusted code, and can schedule events at elevated access levels
        - System calls can then be as fast as native in-process async/await

## Non-goals

- SOS is designed for cluster usage, and as such is not a Desktop OS. There's some interesting design space overlap, for instance consider thinking of an external monitor / smart TV / accelerator / etc. as a distributed service, but for now any sort of graphical or interactive usage is out of scope.

## Implementation
The current implementation has drawn heavily from the [Writing an OS in Rust blog](https://os.phil-opp.com/) as well as a [prior experiment of mine](https://github.com/bethebunny/sos) building a service-oriented "OS" as a python event loop, which demonstrated the power of such an approach with an incredibly ssh-like implementation early in its lifetime.

Much of the current code has been an exercise in me learning Rust, and I don't necessarily expect to index much on keeping the existing code. Rather, I rabbit-holed quite hard on memory allocation implementations and implementing ideas from [Bonwick's paper on arena allocators](https://www.usenix.org/legacy/publications/library/proceedings/usenix01/full_papers/bonwick/bonwick.pdf) before having any practical usage for them.

Now that the project has clearer goals, as outlined above, the next steps in the project are to

### V0 Milestone goals
- [ ] Create a couple simple applications as hero use-cases
    - [ ] Simple web server with some stateful components
    - [ ] Slack bot
- [ ] Use learnings from these apps to define a clearer interface for system and service calls
- [ ] Define an application format, DSL and tooling for declaring applications
- [ ] Core event loop for running applications
- [ ] Barebones system calls for implementing hero applications
- [ ] Develop the kernel to the point of running the applications
- [ ] Demo running on a single machine
- [ ] Demo running on a 2-node cluster with shared resources