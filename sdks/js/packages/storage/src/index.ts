import {
  type RequestContext,
  type Result,
  get,
  post,
  del,
  request,
} from '@reactor/shared';

export interface FileObject {
  name: string;
  id: string;
  bucket_id: string;
  owner?: string;
  created_at: string;
  updated_at: string;
  metadata?: Record<string, unknown>;
}

export interface Bucket {
  id: string;
  name: string;
  public: boolean;
  created_at: string;
  updated_at: string;
}

export interface UploadOptions {
  contentType?: string;
  cacheControl?: string;
  upsert?: boolean;
  metadata?: Record<string, unknown>;
}

export interface ListOptions {
  limit?: number;
  offset?: number;
  sortBy?: { column: string; order: 'asc' | 'desc' };
  search?: string;
}

export interface SignedUrlOptions {
  download?: boolean | string;
  transform?: { width?: number; height?: number; quality?: number };
}

export class StorageBucketClient {
  constructor(
    private ctx: RequestContext,
    private bucketId: string
  ) {}

  async upload(
    path: string,
    file: Blob | ArrayBuffer | File,
    options?: UploadOptions
  ): Promise<Result<{ path: string; id: string }>> {
    const formData = new FormData();
    const blob = file instanceof Blob ? file : new Blob([file]);
    formData.append('file', blob);

    if (options?.metadata) {
      formData.append('metadata', JSON.stringify(options.metadata));
    }

    const headers: Record<string, string> = {};
    if (options?.contentType) headers['Content-Type'] = options.contentType;
    if (options?.cacheControl) headers['Cache-Control'] = options.cacheControl;
    if (options?.upsert) headers['X-Upsert'] = 'true';

    return request(
      this.ctx,
      `/storage/v1/object/${encodeURIComponent(this.bucketId)}/${path}`,
      { method: 'POST', body: formData, headers }
    );
  }

  async download(path: string): Promise<Result<Blob>> {
    return request(
      this.ctx,
      `/storage/v1/object/${encodeURIComponent(this.bucketId)}/${path}`,
      { method: 'GET', responseType: 'blob' }
    );
  }

  async createSignedUrl(
    path: string,
    expiresIn: number,
    options?: SignedUrlOptions
  ): Promise<Result<{ signedUrl: string }>> {
    return post(
      this.ctx,
      `/storage/v1/object/sign/${encodeURIComponent(this.bucketId)}/${path}`,
      { expiresIn, ...options }
    );
  }

  async createSignedUrls(
    paths: string[],
    expiresIn: number
  ): Promise<Result<{ path: string; signedUrl: string }[]>> {
    return post(this.ctx, `/storage/v1/object/sign/${encodeURIComponent(this.bucketId)}`, {
      paths,
      expiresIn,
    });
  }

  getPublicUrl(path: string): string {
    return `${this.ctx.baseUrl}/storage/v1/object/public/${encodeURIComponent(this.bucketId)}/${path}`;
  }

  async list(prefix?: string, options?: ListOptions): Promise<Result<FileObject[]>> {
    const params = new URLSearchParams();
    if (prefix) params.set('prefix', prefix);
    if (options?.limit) params.set('limit', String(options.limit));
    if (options?.offset) params.set('offset', String(options.offset));
    if (options?.search) params.set('search', options.search);

    return get(
      this.ctx,
      `/storage/v1/object/list/${encodeURIComponent(this.bucketId)}?${params}`
    );
  }

  async remove(paths: string[]): Promise<Result<{ name: string }[]>> {
    return del(this.ctx, `/storage/v1/object/${encodeURIComponent(this.bucketId)}`, {
      body: { prefixes: paths },
    });
  }

  async move(from: string, to: string): Promise<Result<{ message: string }>> {
    return post(this.ctx, '/storage/v1/object/move', {
      bucketId: this.bucketId,
      sourceKey: from,
      destinationKey: to,
    });
  }

  async copy(from: string, to: string): Promise<Result<{ path: string }>> {
    return post(this.ctx, '/storage/v1/object/copy', {
      bucketId: this.bucketId,
      sourceKey: from,
      destinationKey: to,
    });
  }
}

export class StorageClient {
  constructor(private ctx: RequestContext) {}

  from(bucketId: string): StorageBucketClient {
    return new StorageBucketClient(this.ctx, bucketId);
  }

  async createBucket(
    id: string,
    options?: { public?: boolean }
  ): Promise<Result<Bucket>> {
    return post(this.ctx, '/storage/v1/bucket', { id, public: options?.public ?? false });
  }

  async deleteBucket(id: string): Promise<Result<void>> {
    return del(this.ctx, `/storage/v1/bucket/${encodeURIComponent(id)}`);
  }

  async listBuckets(): Promise<Result<Bucket[]>> {
    return get(this.ctx, '/storage/v1/bucket');
  }

  async getBucket(id: string): Promise<Result<Bucket>> {
    return get(this.ctx, `/storage/v1/bucket/${encodeURIComponent(id)}`);
  }
}

export function createStorageClient(ctx: RequestContext): StorageClient {
  return new StorageClient(ctx);
}
