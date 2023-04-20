export interface Position {
  line: number,
  character: number
}

export interface Callable {
  class_name: string,
  method_name: string | null
}
