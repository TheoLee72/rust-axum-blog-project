import grpc
from concurrent import futures
import embed_pb2
import embed_pb2_grpc
from sentence_transformers import SentenceTransformer

class EmbedService(embed_pb2_grpc.EmbedServiceServicer):
    def __init__(self):
        print("Initializing SentenceTransformer...")
        self.model = SentenceTransformer('google/embeddinggemma-300m')
        print("SentenceTransformer Initialized.")
        print(f"Embedding model device: {self.model.device}")

    def EmbedQuery(self, request, context):
        instruction = f"{request.task}: "
        embedding = self.model.encode(instruction + request.text)
        return embed_pb2.EmbedReply(embedding=embedding.tolist())


def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=4))
    embed_pb2_grpc.add_EmbedServiceServicer_to_server(EmbedService(), server)
    server.add_insecure_port('[::]:50051')
    server.start()
    print("gRPC EmbedService server running on port 50051...")
    server.wait_for_termination()

if __name__ == "__main__":
    serve()
